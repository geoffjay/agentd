//! macOS platform implementation using LaunchAgent plists and launchctl.

use super::{Platform, ServiceInfo, SERVICES};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct MacOSPlatform;

impl Platform for MacOSPlatform {
    fn install(&self, bin_dir: &Path) -> Result<()> {
        install_binaries(bin_dir)?;

        // Install plist files
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");
        fs::create_dir_all(&plist_dir).context("Failed to create LaunchAgents directory")?;
        install_plists(&plist_dir)?;

        // Setup log directory
        setup_log_directory()?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        // Stop services first
        let _ = self.stop_services();

        // Remove Agent.app bundle
        let app_bundle = PathBuf::from("/Applications/Agent.app");
        if app_bundle.exists() {
            fs::remove_dir_all(&app_bundle).context("Failed to remove Agent.app")?;
            println!("  {} Removed Agent.app", "✓".green());
        }

        // Remove symlink
        let prefix = crate::get_prefix();
        let bin_dir = prefix.join("bin");
        let symlink_path = bin_dir.join("agent");
        if symlink_path.exists() {
            fs::remove_file(&symlink_path).context("Failed to remove agent symlink")?;
            println!("  {} Removed agent symlink", "✓".green());
        }

        // Remove plist files
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");
        for service in SERVICES {
            let plist_name = format!("com.geoffjay.agentd-{}.plist", service.name);
            let plist_path = plist_dir.join(&plist_name);
            if plist_path.exists() {
                fs::remove_file(&plist_path).context(format!("Failed to remove {plist_name}"))?;
                println!("  {} Removed {}", "✓".green(), plist_name);
            }
        }

        Ok(())
    }

    fn start_services(&self) -> Result<()> {
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");

        for service in SERVICES {
            let plist_name = format!("com.geoffjay.agentd-{}.plist", service.name);
            let plist_path = plist_dir.join(&plist_name);

            if plist_path.exists() {
                print!("  Starting agentd-{}... ", service.name);
                let output = Command::new("launchctl")
                    .arg("load")
                    .arg(&plist_path)
                    .output()
                    .context("Failed to execute launchctl")?;

                if output.status.success() {
                    println!("{}", "✓".green());
                } else {
                    println!("{}", "⚠ (may already be running)".yellow());
                }
            }
        }

        Ok(())
    }

    fn stop_services(&self) -> Result<()> {
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");

        for service in SERVICES {
            let plist_name = format!("com.geoffjay.agentd-{}.plist", service.name);
            let plist_path = plist_dir.join(&plist_name);

            if plist_path.exists() {
                print!("  Stopping agentd-{}... ", service.name);
                let output = Command::new("launchctl")
                    .arg("unload")
                    .arg(&plist_path)
                    .output()
                    .context("Failed to execute launchctl")?;

                if output.status.success() {
                    println!("{}", "✓".green());
                } else {
                    println!("{}", "⚠ (may not be running)".yellow());
                }
            }
        }

        Ok(())
    }

    fn start_service(&self, service: &str) -> Result<()> {
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
        let plist_path = plist_dir.join(&plist_name);

        if !plist_path.exists() {
            anyhow::bail!(
                "Service '{service}' not installed. Run 'cargo xtask install-user' first."
            );
        }

        print!("  Starting agentd-{service}... ");
        let output = Command::new("launchctl")
            .arg("load")
            .arg(&plist_path)
            .output()
            .context("Failed to execute launchctl")?;

        if output.status.success() {
            println!("{}", "✓".green());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already loaded") {
                println!("{}", "⚠ (already running)".yellow());
            } else {
                println!("{}", "✗ (failed)".red());
                eprintln!("  Error: {}", stderr.trim());
            }
        }

        Ok(())
    }

    fn stop_service(&self, service: &str) -> Result<()> {
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
        let plist_path = plist_dir.join(&plist_name);

        if !plist_path.exists() {
            anyhow::bail!("Service '{service}' not installed");
        }

        print!("  Stopping agentd-{service}... ");
        let output = Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .output()
            .context("Failed to execute launchctl")?;

        if output.status.success() {
            println!("{}", "✓".green());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Could not find") {
                println!("{}", "⚠ (not running)".yellow());
            } else {
                println!("{}", "✗ (failed)".red());
                eprintln!("  Error: {}", stderr.trim());
            }
        }

        Ok(())
    }

    fn service_status(&self) -> Result<()> {
        let output = Command::new("launchctl")
            .arg("list")
            .output()
            .context("Failed to execute launchctl")?;

        let list_output = String::from_utf8_lossy(&output.stdout);

        for service in SERVICES {
            let service_name = format!("com.geoffjay.agentd-{}", service.name);
            print!("  agentd-{}: ", service.name);

            if list_output.contains(&service_name) {
                println!("{}", "running".green());
            } else {
                println!("{}", "stopped".red());
            }
        }

        Ok(())
    }

    fn print_install_summary(&self) -> Result<()> {
        let plist_dir = crate::home_dir()?.join("Library/LaunchAgents");
        println!("App bundle: {}", "/Applications/Agent.app".yellow());
        println!("CLI symlink: {}", "/usr/local/bin/agent".yellow());
        println!("Services: {}", plist_dir.display().to_string().yellow());
        Ok(())
    }
}

/// Generate a LaunchAgent plist XML string for a service.
#[allow(dead_code)]
pub fn generate_plist(service: &ServiceInfo, bin_path: &Path, log_dir: &Path) -> String {
    let mut env_entries = format!(
        "        <key>RUST_LOG</key>\n        <string>info</string>\n        <key>PORT</key>\n        <string>{}</string>",
        service.port
    );

    for (key, value) in service.extra_env {
        env_entries
            .push_str(&format!("\n        <key>{}</key>\n        <string>{}</string>", key, value));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.geoffjay.agentd-{name}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>

    <key>StandardOutPath</key>
    <string>{log_dir}/agentd-{name}.log</string>

    <key>StandardErrorPath</key>
    <string>{log_dir}/agentd-{name}.err</string>

    <key>EnvironmentVariables</key>
    <dict>
{env}
    </dict>

    <key>WorkingDirectory</key>
    <string>/usr/local</string>
</dict>
</plist>
"#,
        name = service.name,
        bin = bin_path.display(),
        log_dir = log_dir.display(),
        env = env_entries,
    )
}

// -- Private helpers --

fn install_binaries(bin_dir: &Path) -> Result<()> {
    println!("{}", "Installing Agent.app bundle...".blue());

    // Create Agent.app bundle structure
    let app_bundle = PathBuf::from("/Applications/Agent.app");
    let macos_dir = app_bundle.join("Contents/MacOS");
    let resources_dir = app_bundle.join("Contents/Resources");

    fs::create_dir_all(&macos_dir).context("Failed to create Agent.app/Contents/MacOS")?;
    fs::create_dir_all(&resources_dir).context("Failed to create Agent.app/Contents/Resources")?;

    // Install CLI binary to Agent.app/Contents/MacOS/cli
    let cli_src = Path::new("target/release/cli");
    let cli_dest = macos_dir.join("cli");
    if cli_src.exists() {
        fs::copy(cli_src, &cli_dest).context("Failed to install CLI binary")?;
        crate::set_executable(&cli_dest)?;
        println!("  {} CLI binary (cli)", "✓".green());
    } else {
        println!("  {} CLI binary (not built)", "⚠".yellow());
    }

    // Install service binaries to Agent.app/Contents/MacOS/
    for service in SERVICES {
        let src = Path::new("target/release").join(service.binary);
        let dest = macos_dir.join(service.binary);

        if src.exists() {
            fs::copy(&src, &dest).context(format!("Failed to install {}", service.binary))?;
            crate::set_executable(&dest)?;
            println!("  {} {}", "✓".green(), service.binary);
        } else {
            println!("  {} {} (not built)", "⚠".yellow(), service.binary);
        }
    }

    // Create symlink from /usr/local/bin/agent to CLI
    println!();
    println!("{}", "Creating symlink...".blue());

    let symlink_path = bin_dir.join("agent");
    let target_path = macos_dir.join("cli");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let needs_sudo =
            if !bin_dir.exists() { fs::create_dir_all(bin_dir).is_err() } else { false };

        if symlink_path.exists() && fs::remove_file(&symlink_path).is_err() {
            println!("{}", "  Existing symlink requires sudo to remove...".yellow());
            let status = Command::new("sudo")
                .arg("rm")
                .arg(&symlink_path)
                .status()
                .context("Failed to execute sudo rm")?;

            if !status.success() {
                anyhow::bail!("Failed to remove existing symlink");
            }
        }

        let symlink_result = if needs_sudo {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "needs sudo"))
        } else {
            fs::create_dir_all(bin_dir).ok();
            symlink(&target_path, &symlink_path)
        };

        match symlink_result {
            Ok(_) => {
                println!("  {} agent -> {}", "✓".green(), target_path.display());
            }
            Err(_) => {
                println!("{}", "  Creating symlink requires sudo...".yellow());
                let status = Command::new("sudo")
                    .arg("ln")
                    .arg("-sf")
                    .arg(&target_path)
                    .arg(&symlink_path)
                    .status()
                    .context("Failed to execute sudo ln")?;

                if !status.success() {
                    anyhow::bail!("Failed to create symlink with sudo");
                }

                println!(
                    "  {} agent -> {} (created with sudo)",
                    "✓".green(),
                    target_path.display()
                );
            }
        }
    }

    Ok(())
}

fn install_plists(plist_dir: &Path) -> Result<()> {
    println!("{}", "Installing service plists...".blue());

    let plist_src_dir = Path::new("contrib/plists");

    for service in SERVICES {
        let plist_name = format!("com.geoffjay.agentd-{}.plist", service.name);
        let src = plist_src_dir.join(&plist_name);
        let dest = plist_dir.join(&plist_name);

        if src.exists() {
            fs::copy(src, dest).context(format!("Failed to install {plist_name}"))?;
            println!("  {} {}", "✓".green(), plist_name);
        } else {
            println!("  {} {} (not found)", "⚠".yellow(), plist_name);
        }
    }

    Ok(())
}

fn setup_log_directory() -> Result<()> {
    println!();
    println!("{}", "Setting up log directory...".blue());

    let prefix = crate::get_prefix();
    let log_dir = prefix.join("var/log");

    let dir_created = if !log_dir.exists() {
        println!("{}", "  Creating log directory (requires sudo)...".yellow());
        let status = Command::new("sudo")
            .arg("mkdir")
            .arg("-p")
            .arg(&log_dir)
            .status()
            .context("Failed to execute sudo mkdir")?;

        if !status.success() {
            eprintln!("{}", "Error: Failed to create log directory".red());
            return Err(anyhow::anyhow!("Failed to create log directory"));
        }
        true
    } else {
        false
    };

    let user = std::env::var("USER")
        .unwrap_or_else(|_| std::env::var("LOGNAME").unwrap_or_else(|_| "user".to_string()));

    println!("{}", format!("  Ensuring {user} can write to log directory...").blue());

    let chown_status = Command::new("sudo")
        .arg("chown")
        .arg("-R")
        .arg(&user)
        .arg(&log_dir)
        .status()
        .context("Failed to execute sudo chown")?;

    if !chown_status.success() {
        eprintln!("{}", "Warning: Failed to change log directory ownership".yellow());
        eprintln!("Services may not be able to write logs");
        eprintln!("Run manually:");
        eprintln!("  {}", format!("sudo chown -R $(whoami) {}", log_dir.display()).cyan());
    } else if dir_created {
        println!("  {} Log directory created and owned by {}", "✓".green(), user);
    } else {
        println!("  {} Log directory ownership fixed (now owned by {})", "✓".green(), user);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_generate_plist_basic() {
        let info =
            ServiceInfo { name: "notify", binary: "agentd-notify", port: 7004, extra_env: &[] };
        let bin_path = Path::new("/Applications/Agent.app/Contents/MacOS/agentd-notify");
        let log_dir = Path::new("/usr/local/var/log");
        let plist = generate_plist(&info, bin_path, log_dir);

        assert!(plist.contains("com.geoffjay.agentd-notify"));
        assert!(plist.contains("/Applications/Agent.app/Contents/MacOS/agentd-notify"));
        assert!(plist.contains("<string>7004</string>"));
        assert!(plist.contains("RUST_LOG"));
        assert!(plist.contains("<true/>")); // RunAtLoad
        assert!(plist.contains("agentd-notify.log"));
        assert!(plist.contains("agentd-notify.err"));
    }

    #[test]
    fn test_generate_plist_with_extra_env() {
        let info = ServiceInfo {
            name: "ask",
            binary: "agentd-ask",
            port: 7001,
            extra_env: &[("NOTIFY_SERVICE_URL", "http://localhost:7004")],
        };
        let bin_path = Path::new("/Applications/Agent.app/Contents/MacOS/agentd-ask");
        let log_dir = Path::new("/usr/local/var/log");
        let plist = generate_plist(&info, bin_path, log_dir);

        assert!(plist.contains("com.geoffjay.agentd-ask"));
        assert!(plist.contains("<string>7001</string>"));
        assert!(plist.contains("NOTIFY_SERVICE_URL"));
        assert!(plist.contains("http://localhost:7004"));
    }
}
