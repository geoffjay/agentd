mod platform;

use anyhow::{Context, Result};
use colored::Colorize;
use platform::SERVICE_NAMES;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let task = args.get(1).map(|s| s.as_str());

    match task {
        Some("install") => install()?,
        Some("install-user") => install_user()?,
        Some("install-completions") => install_completions()?,
        Some("uninstall") => uninstall()?,
        Some("start-services") => start_services()?,
        Some("stop-services") => stop_services()?,
        Some("restart-services") => restart_services()?,
        Some("service-status") => service_status()?,
        Some("start-service") => {
            let service = args.get(2).context("Service name required")?;
            start_service(service)?;
        }
        Some("stop-service") => {
            let service = args.get(2).context("Service name required")?;
            stop_service(service)?;
        }
        Some("restart-service") => {
            let service = args.get(2).context("Service name required")?;
            restart_service(service)?;
        }
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    println!("{}", "agentd xtask commands:".blue().bold());
    println!();
    println!("{}", "Installation:".cyan());
    println!("  {} - Install for current user", "install-user".green());
    println!("  {} - System-wide install (requires sudo)", "install".green());
    println!(
        "  {} - Generate & install shell completions",
        "install-completions".green()
    );
    println!("  {} - Uninstall all components", "uninstall".green());
    println!();
    println!("{}", "Service Management:".cyan());
    println!("  {} - Start all services", "start-services".green());
    println!("  {} - Stop all services", "stop-services".green());
    println!("  {} - Restart all services", "restart-services".green());
    println!("  {} <name> - Start specific service", "start-service".green());
    println!("  {} <name> - Stop specific service", "stop-service".green());
    println!("  {} <name> - Restart specific service", "restart-service".green());
    println!("  {} - Check service status", "service-status".green());
    println!();
    println!("{}", "Examples:".cyan());
    println!("  {}", "cargo xtask install-user".yellow());
    println!("  {}", "cargo xtask start-service notify".yellow());
    println!("  {}", "cargo xtask restart-service ask".yellow());
    println!();
    println!("{}", "Available services:".cyan());
    println!("  {}", SERVICE_NAMES.join(", "));
    println!();
    println!(
        "{}: {}",
        "Platform".cyan(),
        if cfg!(target_os = "macos") {
            "macOS (launchd)"
        } else if cfg!(target_os = "linux") {
            "Linux (systemd)"
        } else {
            "unknown"
        }
    );
}

fn install_user() -> Result<()> {
    println!("{}", "Installing agentd (user mode)...".blue().bold());
    println!();

    check_in_project_root()?;

    // Build binaries
    println!("{}", "Building binaries...".blue());
    build_release()?;

    // Determine install prefix and bin directory
    let prefix = get_prefix();
    let bin_dir = prefix.join("bin");

    // Try to create bin directory, give helpful message if it fails
    if let Err(e) = fs::create_dir_all(&bin_dir) {
        eprintln!("{}", format!("Failed to create directory: {}", bin_dir.display()).red());
        eprintln!("{}", "To fix permissions, run:".yellow());
        eprintln!("  {}", format!("sudo mkdir -p {}", bin_dir.display()).cyan());
        eprintln!("  {}", format!("sudo chown -R $(whoami) {}", prefix.display()).cyan());
        return Err(e.into());
    }

    // Delegate to platform-specific installer
    let plat = platform::detect_platform();
    plat.install(&bin_dir)?;

    // Install shell completions
    println!();
    if let Err(e) = install_completions() {
        eprintln!(
            "{}",
            format!("Warning: Failed to install shell completions: {}", e).yellow()
        );
    }

    println!();
    println!("{}", "✓ Installation complete!".green().bold());
    println!();
    plat.print_install_summary()?;
    println!();
    println!("{}", "Usage:".cyan().bold());
    println!("  {} - List notifications", "agent notify list".cyan());
    println!(
        "  {} - Create notification",
        "agent notify create --title \"Test\" --message \"Hello\"".cyan()
    );
    println!();
    println!("To start services: {}", "cargo xtask start-services".cyan());

    Ok(())
}

fn install() -> Result<()> {
    println!("{}", "Note: System-wide installation requires sudo".yellow());
    println!("Consider using 'install-user' instead.");
    println!();
    install_user()
}

fn uninstall() -> Result<()> {
    println!("{}", "Uninstalling agentd...".blue().bold());

    let plat = platform::detect_platform();
    plat.uninstall()?;

    println!();
    println!("{}", "✓ Uninstallation complete!".green().bold());

    Ok(())
}

fn start_services() -> Result<()> {
    println!("{}", "Starting services...".blue());

    let plat = platform::detect_platform();
    plat.start_services()?;

    println!();
    println!("{}", "✓ Services started".green().bold());
    Ok(())
}

fn stop_services() -> Result<()> {
    println!("{}", "Stopping services...".blue());

    let plat = platform::detect_platform();
    plat.stop_services()?;

    println!();
    println!("{}", "✓ Services stopped".green().bold());
    Ok(())
}

fn service_status() -> Result<()> {
    println!("{}", "Service Status:".blue().bold());
    println!();

    let plat = platform::detect_platform();
    plat.service_status()?;

    Ok(())
}

fn start_service(service: &str) -> Result<()> {
    validate_service_name(service)?;

    let plat = platform::detect_platform();
    plat.start_service(service)?;

    Ok(())
}

fn stop_service(service: &str) -> Result<()> {
    validate_service_name(service)?;

    let plat = platform::detect_platform();
    plat.stop_service(service)?;

    Ok(())
}

fn restart_services() -> Result<()> {
    println!("{}", "Restarting all services...".blue());
    println!();

    stop_services()?;
    println!();
    start_services()?;

    Ok(())
}

fn restart_service(service: &str) -> Result<()> {
    validate_service_name(service)?;

    println!("{}", format!("Restarting agentd-{service}...").blue());

    let plat = platform::detect_platform();
    plat.stop_service(service)?;
    plat.start_service(service)?;

    println!();
    println!("{}", format!("✓ Service agentd-{service} restarted").green().bold());

    Ok(())
}

fn install_completions() -> Result<()> {
    println!(
        "{}",
        "Installing shell completions...".blue().bold()
    );
    println!();

    let bin_dir = get_prefix().join("bin");
    let agent_bin = bin_dir.join("agent");

    // Ensure the agent binary exists (either installed or in target/)
    let agent_cmd = if agent_bin.exists() {
        agent_bin.to_string_lossy().to_string()
    } else {
        // Try the release build
        let release_bin = Path::new("target/release/cli");
        if release_bin.exists() {
            release_bin.to_string_lossy().to_string()
        } else {
            println!(
                "{}",
                "agent binary not found. Building...".yellow()
            );
            build_release()?;
            "target/release/cli".to_string()
        }
    };

    let home = home_dir()?;

    // Bash completions
    let bash_dir = home.join(".local/share/bash-completion/completions");
    if let Err(e) = fs::create_dir_all(&bash_dir) {
        eprintln!("  {} bash: {}", "⚠".yellow(), e);
    } else {
        let output = Command::new(&agent_cmd)
            .args(["completions", "bash"])
            .output()
            .context("Failed to generate bash completions")?;
        if output.status.success() {
            fs::write(bash_dir.join("agent"), &output.stdout)?;
            println!("  {} bash → {}", "✓".green(), bash_dir.join("agent").display());
        }
    }

    // Zsh completions
    let zsh_dir = home.join(".zfunc");
    if let Err(e) = fs::create_dir_all(&zsh_dir) {
        eprintln!("  {} zsh: {}", "⚠".yellow(), e);
    } else {
        let output = Command::new(&agent_cmd)
            .args(["completions", "zsh"])
            .output()
            .context("Failed to generate zsh completions")?;
        if output.status.success() {
            fs::write(zsh_dir.join("_agent"), &output.stdout)?;
            println!("  {} zsh  → {}", "✓".green(), zsh_dir.join("_agent").display());
        }
    }

    // Fish completions
    let fish_dir = home.join(".config/fish/completions");
    if let Err(e) = fs::create_dir_all(&fish_dir) {
        eprintln!("  {} fish: {}", "⚠".yellow(), e);
    } else {
        let output = Command::new(&agent_cmd)
            .args(["completions", "fish"])
            .output()
            .context("Failed to generate fish completions")?;
        if output.status.success() {
            fs::write(fish_dir.join("agent.fish"), &output.stdout)?;
            println!(
                "  {} fish → {}",
                "✓".green(),
                fish_dir.join("agent.fish").display()
            );
        }
    }

    println!();
    println!("{}", "✓ Shell completions installed!".green().bold());
    println!();
    println!("{}", "Note: You may need to restart your shell or source the completions.".bright_black());
    println!(
        "{}",
        "For zsh, ensure ~/.zfunc is in your fpath: fpath=(~/.zfunc $fpath)".bright_black()
    );

    Ok(())
}

// === Shared helpers (used by platform modules via crate::) ===

pub fn check_in_project_root() -> Result<()> {
    if !Path::new("Cargo.toml").exists() || !Path::new("crates").exists() {
        anyhow::bail!("Must be run from the agentd project root");
    }
    Ok(())
}

pub fn build_release() -> Result<()> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--workspace")
        .arg("--bins")
        .status()
        .context("Failed to execute cargo build")?;

    if !status.success() {
        anyhow::bail!("Build failed");
    }

    Ok(())
}

pub fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

pub fn get_prefix() -> PathBuf {
    env::var("PREFIX").map(PathBuf::from).unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            PathBuf::from("/usr/local")
        } else {
            // Linux: default to ~/.local for user installs
            home_dir().unwrap_or_else(|_| PathBuf::from("/usr/local")).join(".local")
        }
    })
}

pub fn home_dir() -> Result<PathBuf> {
    env::var("HOME").map(PathBuf::from).context("HOME environment variable not set")
}

pub fn validate_service_name(service: &str) -> Result<()> {
    if !SERVICE_NAMES.contains(&service) {
        anyhow::bail!(
            "Invalid service name: '{}'. Valid services are: {}",
            service,
            SERVICE_NAMES.join(", ")
        );
    }
    Ok(())
}
