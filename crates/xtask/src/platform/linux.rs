//! Linux platform implementation using systemd user or system units.
//!
//! When running as root (`is_root()` returns true), all paths and systemd
//! scope are system-wide:
//!   binaries  → /usr/local/bin
//!   assets    → /usr/local/share/agentd
//!   units     → /etc/systemd/system  (WantedBy=multi-user.target)
//!
//! Otherwise the per-user XDG layout is used:
//!   binaries  → ~/.local/bin
//!   assets    → XDG_DATA_HOME/agentd
//!   units     → XDG_CONFIG_HOME/systemd/user  (WantedBy=default.target)

use super::{Platform, ServiceInfo, SERVICES};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LinuxPlatform {
    pub system: bool,
}

impl LinuxPlatform {
    /// Build a `systemctl` command pre-populated with `--user` when not a system install.
    fn systemctl(&self) -> Command {
        let mut cmd = Command::new("systemctl");
        if !self.system {
            cmd.arg("--user");
        }
        cmd
    }

    fn systemd_dir(&self) -> PathBuf {
        if self.system {
            PathBuf::from("/etc/systemd/system")
        } else {
            let config_home = std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| crate::home_dir().unwrap_or_default().join(".config"));
            config_home.join("systemd/user")
        }
    }

    fn ui_assets_dir(&self) -> Result<PathBuf> {
        if self.system {
            Ok(PathBuf::from("/usr/local/share/agentd/ui"))
        } else {
            let data_home = std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| crate::home_dir().unwrap_or_default().join(".local/share"));
            Ok(data_home.join("agentd/ui"))
        }
    }

    fn log_directory(&self) -> Result<PathBuf> {
        if self.system {
            Ok(PathBuf::from("/usr/local/share/agentd/log"))
        } else {
            let data_home = std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| crate::home_dir().unwrap_or_default().join(".local/share"));
            Ok(data_home.join("agentd/log"))
        }
    }
}

impl Platform for LinuxPlatform {
    fn install(&self, bin_dir: &Path) -> Result<()> {
        install_binaries(bin_dir)?;

        // Install UI assets
        self.install_ui_assets()?;

        // Generate and install systemd unit files
        let unit_dir = self.systemd_dir();
        fs::create_dir_all(&unit_dir).context("Failed to create systemd directory")?;
        self.install_unit_files(&unit_dir, bin_dir)?;

        // Reload systemd daemon
        println!();
        println!("{}", "Reloading systemd daemon...".blue());
        let status = self
            .systemctl()
            .arg("daemon-reload")
            .status()
            .context("Failed to execute systemctl daemon-reload")?;

        if status.success() {
            println!("  {} systemd daemon reloaded", "✓".green());
        } else {
            eprintln!("{}", "Warning: Failed to reload systemd daemon".yellow());
        }

        // Setup log directory
        self.setup_log_directory()?;

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
        let unit_dir = self.systemd_dir();
        if unit_dir.exists() {
            for service in SERVICES {
                let unit_name = format!("agentd-{}.service", service.name);
                let unit_path = unit_dir.join(&unit_name);
                if unit_path.exists() {
                    fs::remove_file(&unit_path).context(format!("Failed to remove {unit_name}"))?;
                    println!("  {} Removed {}", "✓".green(), unit_name);
                }
            }

            // Reload daemon after removing units
            let _ = self.systemctl().arg("daemon-reload").status();
        }

        Ok(())
    }

    fn start_services(&self) -> Result<()> {
        for service in SERVICES {
            let unit_name = format!("agentd-{}.service", service.name);
            print!("  Starting {}... ", unit_name);

            let output = self
                .systemctl()
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

            let output = self
                .systemctl()
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

        let output = self
            .systemctl()
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

        let output = self
            .systemctl()
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

            let output = self.systemctl().arg("is-active").arg(&unit_name).output();

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
        let unit_dir = self.systemd_dir();
        println!("Binaries: {}", prefix.join("bin").display().to_string().yellow());
        println!("CLI symlink: {}", prefix.join("bin/agent").display().to_string().yellow());
        println!("Unit files: {}", unit_dir.display().to_string().yellow());
        Ok(())
    }
}

impl LinuxPlatform {
    fn install_ui_assets(&self) -> Result<()> {
        println!("{}", "Installing UI assets...".blue());

        let ui_dist = Path::new("ui/dist");
        if !ui_dist.exists() {
            println!("  {} UI dist not found (ui/dist/) — skipping", "⚠".yellow());
            return Ok(());
        }

        let dest = self.ui_assets_dir()?;
        if dest.exists() {
            fs::remove_dir_all(&dest).context("Failed to remove old UI assets")?;
        }
        copy_dir_recursive(ui_dist, &dest)?;

        println!("  {} UI assets installed to {}", "✓".green(), dest.display());
        Ok(())
    }

    fn install_unit_files(&self, unit_dir: &Path, bin_dir: &Path) -> Result<()> {
        let scope = if self.system { "system" } else { "user" };
        println!("{}", format!("Installing systemd {scope} units...").blue());

        for service in SERVICES {
            let bin_path = bin_dir.join(service.binary);
            let mut unit_content = generate_unit_file(service, &bin_path, self.system);

            // Add AGENTD_UI_DIR for the ui service
            if service.name == "ui" {
                if let Ok(ui_dir) = self.ui_assets_dir() {
                    let env_line = format!("\nEnvironment=AGENTD_UI_DIR={}", ui_dir.display());
                    unit_content =
                        unit_content.replace("\n\n[Install]", &format!("{env_line}\n\n[Install]"));
                }
            }

            let unit_name = format!("agentd-{}.service", service.name);
            let unit_path = unit_dir.join(&unit_name);

            fs::write(&unit_path, &unit_content).context(format!("Failed to write {unit_name}"))?;
            println!("  {} {}", "✓".green(), unit_name);
        }

        Ok(())
    }

    fn setup_log_directory(&self) -> Result<()> {
        let log_dir = self.log_directory()?;

        if !log_dir.exists() {
            println!();
            println!("{}", "Setting up log directory...".blue());
            fs::create_dir_all(&log_dir)
                .context(format!("Failed to create log directory: {}", log_dir.display()))?;
            println!("  {} Log directory created at {}", "✓".green(), log_dir.display());
        }

        Ok(())
    }
}

/// Generate a systemd unit file for a service.
///
/// `system` controls the `WantedBy=` target: `multi-user.target` for system
/// units, `default.target` for user units.
pub fn generate_unit_file(service: &ServiceInfo, bin_path: &Path, system: bool) -> String {
    let mut env_lines =
        format!("Environment=RUST_LOG=info\nEnvironment=AGENTD_PORT={}", service.port);

    for (key, value) in service.extra_env {
        env_lines.push_str(&format!("\nEnvironment={}={}", key, value));
    }

    let wanted_by = if system { "multi-user.target" } else { "default.target" };

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
WantedBy={wanted_by}
"#,
        name = service.name,
        bin = bin_path.display(),
        env = env_lines,
    )
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

/// Recursively copy a directory tree.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).context(format!("Failed to create directory: {}", dest.display()))?;

    for entry in
        fs::read_dir(src).context(format!("Failed to read directory: {}", src.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .context(format!("Failed to copy {}", src_path.display()))?;
        }
    }

    Ok(())
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
        let unit = generate_unit_file(&info, bin_path, false);

        assert!(unit.contains("Description=agentd-notify service"));
        assert!(unit.contains("ExecStart=/home/user/.local/bin/agentd-notify"));
        assert!(unit.contains("Environment=AGENTD_PORT=7004"));
        assert!(unit.contains("Environment=RUST_LOG=info"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("[Install]"));
    }

    #[test]
    fn test_generate_unit_file_system_mode() {
        let info =
            ServiceInfo { name: "notify", binary: "agentd-notify", port: 7004, extra_env: &[] };
        let bin_path = Path::new("/usr/local/bin/agentd-notify");
        let unit = generate_unit_file(&info, bin_path, true);

        assert!(unit.contains("WantedBy=multi-user.target"));
        assert!(unit.contains("ExecStart=/usr/local/bin/agentd-notify"));
    }

    #[test]
    fn test_generate_unit_file_with_extra_env() {
        let info = ServiceInfo {
            name: "ask",
            binary: "agentd-ask",
            port: 7001,
            extra_env: &[("AGENTD_NOTIFY_SERVICE_URL", "http://localhost:7004")],
        };
        let bin_path = Path::new("/usr/local/bin/agentd-ask");
        let unit = generate_unit_file(&info, bin_path, true);

        assert!(unit.contains("Description=agentd-ask service"));
        assert!(unit.contains("Environment=AGENTD_PORT=7001"));
        assert!(unit.contains("Environment=AGENTD_NOTIFY_SERVICE_URL=http://localhost:7004"));
    }

    #[test]
    fn test_generate_unit_file_all_services() {
        for service in SERVICES {
            let bin_path = PathBuf::from(format!("/usr/local/bin/{}", service.binary));
            let unit = generate_unit_file(service, &bin_path, true);

            assert!(unit.contains(&format!("Description=agentd-{} service", service.name)));
            assert!(unit.contains(&format!("ExecStart=/usr/local/bin/{}", service.binary)));
            assert!(unit.contains(&format!("Environment=AGENTD_PORT={}", service.port)));
        }
    }
}
