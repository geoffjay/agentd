//! Installation and service-management command implementations.
//!
//! These commands let a production host install agentd and manage its services
//! without a source tree or `cargo`/`bun`. The heavy lifting (writing launchd
//! plists / systemd units, gap-filling the config file, running migrations)
//! lives in the `agentd-install` crate, which is shared with `xtask`.
//!
//! # Examples
//!
//! ```bash
//! agent install                       # install services from the binaries on disk
//! agent install --ui-dir ./ui/dist    # also install bundled UI assets
//! agent service start                 # start all services
//! agent service status                # show service state
//! agent migrate                       # apply pending DB migrations
//! ```

use agentd_install::{detect_platform, migrate, validate_service_name, InstallPaths};
use anyhow::{Context, Result};
use clap::Subcommand;
use clap_complete::Shell;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// Service lifecycle subcommands (`agent service …`).
#[derive(Subcommand)]
pub enum ServiceCommand {
    /// Start all services, or a single service by name.
    Start {
        /// Service name (omit to start all).
        service: Option<String>,
    },
    /// Stop all services, or a single service by name.
    Stop {
        /// Service name (omit to stop all).
        service: Option<String>,
    },
    /// Restart all services, or a single service by name.
    Restart {
        /// Service name (omit to restart all).
        service: Option<String>,
    },
    /// Show the running state of all services.
    Status,
}

impl ServiceCommand {
    pub fn execute(&self) -> Result<()> {
        let plat = detect_platform();
        match self {
            ServiceCommand::Start { service: Some(name) } => {
                validate_service_name(name)?;
                plat.start_service(name)?;
            }
            ServiceCommand::Start { service: None } => {
                println!("{}", "Starting services...".blue());
                plat.start_services()?;
            }
            ServiceCommand::Stop { service: Some(name) } => {
                validate_service_name(name)?;
                plat.stop_service(name)?;
            }
            ServiceCommand::Stop { service: None } => {
                println!("{}", "Stopping services...".blue());
                plat.stop_services()?;
            }
            ServiceCommand::Restart { service: Some(name) } => {
                validate_service_name(name)?;
                println!("{}", format!("Restarting agentd-{name}...").blue());
                plat.stop_service(name)?;
                plat.start_service(name)?;
            }
            ServiceCommand::Restart { service: None } => {
                println!("{}", "Restarting all services...".blue());
                plat.stop_services()?;
                println!();
                plat.start_services()?;
            }
            ServiceCommand::Status => {
                println!("{}", "Service Status:".blue().bold());
                println!();
                plat.service_status()?;
            }
        }
        Ok(())
    }
}

/// Run `agent install`.
///
/// `bin_src` defaults to the directory containing the running `agent` binary,
/// which is where the installer places the downloaded service binaries.
pub async fn run_install(
    bin_src: Option<PathBuf>,
    ui_dir: Option<PathBuf>,
    skip_migrations: bool,
    mut cmd: clap::Command,
) -> Result<()> {
    println!("{}", "Installing agentd...".blue().bold());
    println!();

    let bin_src = match bin_src {
        Some(p) => p,
        None => current_exe_dir().context("Could not determine the agent binary directory")?,
    };

    let prefix = agentd_install::get_prefix();
    let bin_dir = prefix.join("bin");
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("Failed to create bin directory: {}", bin_dir.display()))?;

    let plat = detect_platform();
    plat.install(&InstallPaths {
        bin_src: &bin_src,
        bin_dir: &bin_dir,
        ui_src: ui_dir.as_deref(),
    })?;

    // Shell completions (best-effort — never fail the install over them).
    println!();
    if let Err(e) = install_completions(&mut cmd) {
        eprintln!("  {} shell completions: {}", "⚠".yellow(), e);
    }

    // Apply database migrations so services start with an up-to-date schema.
    if skip_migrations {
        println!();
        println!("  {} skipping migrations (--skip-migrations)", "⚠".yellow());
    } else {
        println!();
        migrate::migrate(None).await?;
    }

    println!();
    println!("{}", "✓ Installation complete!".green().bold());
    println!();
    plat.print_install_summary()?;
    println!();
    println!("To start services: {}", "agent service start".cyan());

    Ok(())
}

/// Run `agent uninstall`.
pub fn run_uninstall() -> Result<()> {
    println!("{}", "Uninstalling agentd...".blue().bold());
    detect_platform().uninstall()?;
    println!();
    println!("{}", "✓ Uninstallation complete!".green().bold());
    Ok(())
}

/// Directory containing the currently-running executable.
fn current_exe_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe() failed")?;
    let dir = exe.parent().context("executable has no parent directory")?;
    Ok(dir.to_path_buf())
}

/// Generate and install shell completions for bash, zsh, and fish.
///
/// Generation happens in-process via clap rather than shelling out to the
/// `agent` binary. Failures for an individual shell are reported but do not
/// abort the others.
fn install_completions(cmd: &mut clap::Command) -> Result<()> {
    println!("{}", "Installing shell completions...".blue());

    let home = agentd_install::home_dir()?;

    // (shell, target directory, file name)
    let targets = [
        (Shell::Bash, home.join(".local/share/bash-completion/completions"), "agent"),
        (Shell::Zsh, home.join(".zfunc"), "_agent"),
        (Shell::Fish, home.join(".config/fish/completions"), "agent.fish"),
    ];

    for (shell, dir, file) in targets {
        if let Err(e) = write_completion(shell, cmd, &dir, file) {
            eprintln!("  {} {:?}: {}", "⚠".yellow(), shell, e);
        } else {
            println!("  {} {:?} → {}", "✓".green(), shell, dir.join(file).display());
        }
    }

    Ok(())
}

fn write_completion(shell: Shell, cmd: &mut clap::Command, dir: &Path, file: &str) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut buf: Vec<u8> = Vec::new();
    clap_complete::generate(shell, cmd, "agent", &mut buf);
    fs::write(dir.join(file), &buf)
        .with_context(|| format!("writing {}", dir.join(file).display()))?;
    Ok(())
}
