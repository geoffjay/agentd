//! Linux platform implementation using systemd user units.

use super::{Platform, ServiceInfo, SERVICES};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LinuxPlatform;

impl Platform for LinuxPlatform {
    fn install(&self, bin_dir: &Path) -> Result<()> {
        install_binaries(bin_dir)?;

        // Generate and install systemd unit files
        let unit_dir = systemd_user_dir()?;
        fs::create_dir_all(&unit_dir).context("Failed to create systemd user directory")?;
        install_unit_files(&unit_dir, bin_dir)?;

        // Reload systemd daemon
        println!();
        println!("{}", "Reloading systemd daemon...".blue());
        let status = Command::new("systemctl")
            .arg("--user")
            .arg("daemon-reload")
            .status()
            .context("Failed to execute systemctl daemon-reload")?;

        if status.success() {
            println!("  {} systemd daemon reloaded", "✓".green());
        } else {
            eprintln!("{}", "Warning: Failed to reload systemd daemon".yellow());
        }

        // Setup log directory
        setup_log_directory()?;

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        // Stop and disable services first
        let _ = self.stop_services();

        // Remove binaries
        let prefix = crate::get_prefix();
        let bin_dir = prefix.join("bin");

        for service in SERVICES {
            let bin_path = bin_dir.join(service.binary);
            if bin_path.exists() {
                fs::remove_file(&bin_path)
                    .context(format!("Failed to remove {}", service.binary))?;
                println!("  {} Removed {}", "✓".green(), service.binary);
            }
        }

        // Remove CLI binary and symlink
        let cli_path = bin_dir.join("cli");
        if cli_path.exists() {
            fs::remove_file(&cli_path).context("Failed to remove cli binary")?;
            println!("  {} Removed cli", "✓".green());
        }
        let symlink_path = bin_dir.join("agent");
        if symlink_path.exists() {
            fs::remove_file(&symlink_path).context("Failed to remove agent symlink")?;
            println!("  {} Removed agent symlink", "✓".green());
        }

        // Remove unit files
        if let Ok(unit_dir) = systemd_user_dir() {
            for service in SERVICES {
                let unit_name = format!("agentd-{}.service", service.name);
                let unit_path = unit_dir.join(&unit_name);
                if unit_path.exists() {
                    fs::remove_file(&unit_path).context(format!("Failed to remove {unit_name}"))?;
                    println!("  {} Removed {}", "✓".green(), unit_name);
                }
            }

            // Reload daemon after removing units
            let _ = Command::new("systemctl").arg("--user").arg("daemon-reload").status();
        }

        Ok(())
    }

    fn start_services(&self) -> Result<()> {
        for service in SERVICES {
            let unit_name = format!("agentd-{}.service", service.name);
            print!("  Starting {}... ", unit_name);

            let output = Command::new("systemctl")
                .arg("--user")
                .arg("start")
                .arg(&unit_name)
                .output()
                .context("Failed to execute systemctl")?;

            if output.status.success() {
                println!("{}", "✓".green());
            } else {
                println!("{}", "⚠ (may already be running)".yellow());
            }
        }

        Ok(())
    }

    fn stop_services(&self) -> Result<()> {
        for service in SERVICES {
            let unit_name = format!("agentd-{}.service", service.name);
            print!("  Stopping {}... ", unit_name);

            let output = Command::new("systemctl")
                .arg("--user")
                .arg("stop")
                .arg(&unit_name)
                .output()
                .context("Failed to execute systemctl")?;

            if output.status.success() {
                println!("{}", "✓".green());
            } else {
                println!("{}", "⚠ (may not be running)".yellow());
            }
        }

        Ok(())
    }

    fn start_service(&self, service: &str) -> Result<()> {
        let unit_name = format!("agentd-{service}.service");
        print!("  Starting {unit_name}... ");

        let output = Command::new("systemctl")
            .arg("--user")
            .arg("start")
            .arg(&unit_name)
            .output()
            .context("Failed to execute systemctl")?;

        if output.status.success() {
            println!("{}", "✓".green());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("{}", "✗ (failed)".red());
            eprintln!("  Error: {}", stderr.trim());
        }

        Ok(())
    }

    fn stop_service(&self, service: &str) -> Result<()> {
        let unit_name = format!("agentd-{service}.service");
        print!("  Stopping {unit_name}... ");

        let output = Command::new("systemctl")
            .arg("--user")
            .arg("stop")
            .arg(&unit_name)
            .output()
            .context("Failed to execute systemctl")?;

        if output.status.success() {
            println!("{}", "✓".green());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not loaded") || stderr.contains("not found") {
                println!("{}", "⚠ (not running)".yellow());
            } else {
                println!("{}", "✗ (failed)".red());
                eprintln!("  Error: {}", stderr.trim());
            }
        }

        Ok(())
    }

    fn service_status(&self) -> Result<()> {
        for service in SERVICES {
            let unit_name = format!("agentd-{}.service", service.name);
            print!("  agentd-{}: ", service.name);

            let output =
                Command::new("systemctl").arg("--user").arg("is-active").arg(&unit_name).output();

            match output {
                Ok(out) => {
                    let status = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if status == "active" {
                        println!("{}", "running".green());
                    } else {
                        println!("{}", "stopped".red());
                    }
                }
                Err(_) => {
                    println!("{}", "unknown".yellow());
                }
            }
        }

        Ok(())
    }

    fn print_install_summary(&self) -> Result<()> {
        let prefix = crate::get_prefix();
        let unit_dir = systemd_user_dir()?;
        println!("Binaries: {}", prefix.join("bin").display().to_string().yellow());
        println!("CLI symlink: {}", prefix.join("bin/agent").display().to_string().yellow());
        println!("Unit files: {}", unit_dir.display().to_string().yellow());
        Ok(())
    }
}

/// Generate a systemd user unit file for a service.
pub fn generate_unit_file(service: &ServiceInfo, bin_path: &Path) -> String {
    let mut env_lines = format!("Environment=RUST_LOG=info\nEnvironment=PORT={}", service.port);

    for (key, value) in service.extra_env {
        env_lines.push_str(&format!("\nEnvironment={}={}", key, value));
    }

    format!(
        r#"[Unit]
Description=agentd-{name} service
After=network.target

[Service]
Type=simple
ExecStart={bin}
Restart=on-failure
RestartSec=5
{env}

[Install]
WantedBy=default.target
"#,
        name = service.name,
        bin = bin_path.display(),
        env = env_lines,
    )
}

/// Get the systemd user unit directory.
fn systemd_user_dir() -> Result<PathBuf> {
    // Respect XDG_CONFIG_HOME, fall back to ~/.config
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::home_dir().unwrap_or_default().join(".config"));

    Ok(config_home.join("systemd/user"))
}

// -- Private helpers --

fn install_binaries(bin_dir: &Path) -> Result<()> {
    println!("{}", "Installing binaries...".blue());

    fs::create_dir_all(bin_dir)
        .context(format!("Failed to create bin directory: {}", bin_dir.display()))?;

    // Install CLI binary
    let cli_src = Path::new("target/release/cli");
    let cli_dest = bin_dir.join("cli");
    if cli_src.exists() {
        fs::copy(cli_src, &cli_dest).context("Failed to install CLI binary")?;
        crate::set_executable(&cli_dest)?;
        println!("  {} CLI binary (cli)", "✓".green());
    } else {
        println!("  {} CLI binary (not built)", "⚠".yellow());
    }

    // Install service binaries
    for service in SERVICES {
        let src = Path::new("target/release").join(service.binary);
        let dest = bin_dir.join(service.binary);

        if src.exists() {
            fs::copy(&src, &dest).context(format!("Failed to install {}", service.binary))?;
            crate::set_executable(&dest)?;
            println!("  {} {}", "✓".green(), service.binary);
        } else {
            println!("  {} {} (not built)", "⚠".yellow(), service.binary);
        }
    }

    // Create agent symlink
    println!();
    println!("{}", "Creating symlink...".blue());

    let symlink_path = bin_dir.join("agent");
    let target_path = cli_dest;

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        if symlink_path.exists() {
            fs::remove_file(&symlink_path).ok();
        }

        match symlink(&target_path, &symlink_path) {
            Ok(_) => {
                println!("  {} agent -> {}", "✓".green(), target_path.display());
            }
            Err(e) => {
                eprintln!("  {} Failed to create symlink: {}", "⚠".yellow(), e);
            }
        }
    }

    Ok(())
}

fn install_unit_files(unit_dir: &Path, bin_dir: &Path) -> Result<()> {
    println!("{}", "Installing systemd user units...".blue());

    for service in SERVICES {
        let bin_path = bin_dir.join(service.binary);
        let unit_content = generate_unit_file(service, &bin_path);
        let unit_name = format!("agentd-{}.service", service.name);
        let unit_path = unit_dir.join(&unit_name);

        fs::write(&unit_path, &unit_content).context(format!("Failed to write {unit_name}"))?;
        println!("  {} {}", "✓".green(), unit_name);
    }

    Ok(())
}

fn setup_log_directory() -> Result<()> {
    let log_dir = log_directory()?;

    if !log_dir.exists() {
        println!();
        println!("{}", "Setting up log directory...".blue());
        fs::create_dir_all(&log_dir)
            .context(format!("Failed to create log directory: {}", log_dir.display()))?;
        println!("  {} Log directory created at {}", "✓".green(), log_dir.display());
    }

    Ok(())
}

/// Get the log directory for Linux.
fn log_directory() -> Result<PathBuf> {
    let data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::home_dir().unwrap_or_default().join(".local/share"));

    Ok(data_home.join("agentd/log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_generate_unit_file_basic() {
        let info =
            ServiceInfo { name: "notify", binary: "agentd-notify", port: 7004, extra_env: &[] };
        let bin_path = Path::new("/home/user/.local/bin/agentd-notify");
        let unit = generate_unit_file(&info, bin_path);

        assert!(unit.contains("Description=agentd-notify service"));
        assert!(unit.contains("ExecStart=/home/user/.local/bin/agentd-notify"));
        assert!(unit.contains("Environment=PORT=7004"));
        assert!(unit.contains("Environment=RUST_LOG=info"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("[Install]"));
    }

    #[test]
    fn test_generate_unit_file_with_extra_env() {
        let info = ServiceInfo {
            name: "ask",
            binary: "agentd-ask",
            port: 7001,
            extra_env: &[("NOTIFY_SERVICE_URL", "http://localhost:7004")],
        };
        let bin_path = Path::new("/usr/local/bin/agentd-ask");
        let unit = generate_unit_file(&info, bin_path);

        assert!(unit.contains("Description=agentd-ask service"));
        assert!(unit.contains("Environment=PORT=7001"));
        assert!(unit.contains("Environment=NOTIFY_SERVICE_URL=http://localhost:7004"));
    }

    #[test]
    fn test_generate_unit_file_all_services() {
        for service in SERVICES {
            let bin_path = PathBuf::from(format!("/usr/local/bin/{}", service.binary));
            let unit = generate_unit_file(service, &bin_path);

            assert!(unit.contains(&format!("Description=agentd-{} service", service.name)));
            assert!(unit.contains(&format!("ExecStart=/usr/local/bin/{}", service.binary)));
            assert!(unit.contains(&format!("Environment=PORT={}", service.port)));
        }
    }
}
