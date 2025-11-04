use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("install") => install()?,
        Some("install-user") => install_user()?,
        Some("uninstall") => uninstall()?,
        Some("start-services") => start_services()?,
        Some("stop-services") => stop_services()?,
        Some("service-status") => service_status()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    println!("{}", "agentd xtask commands:".blue().bold());
    println!();
    println!("  {} - Install for current user", "install-user".green());
    println!("  {} - System-wide install (requires sudo)", "install".green());
    println!("  {} - Uninstall all components", "uninstall".green());
    println!("  {} - Start all services", "start-services".green());
    println!("  {} - Stop all services", "stop-services".green());
    println!("  {} - Check service status", "service-status".green());
    println!();
    println!("Example:");
    println!("  {}", "cargo xtask install-user".yellow());
}

fn install_user() -> Result<()> {
    println!("{}", "Installing agentd (user mode)...".blue().bold());
    println!();

    // Check prerequisites
    check_macos()?;
    check_in_project_root()?;

    // Build binaries
    println!("{}", "Building binaries...".blue());
    build_release()?;

    // Install binaries
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

    install_binaries(&bin_dir)?;

    // Install plist files
    let plist_dir = home_dir()?.join("Library/LaunchAgents");
    fs::create_dir_all(&plist_dir).context("Failed to create LaunchAgents directory")?;

    install_plists(&plist_dir)?;

    // Create log directory
    let log_dir = prefix.join("var/log");
    if let Err(_e) = fs::create_dir_all(&log_dir) {
        eprintln!("{}", format!("Warning: Failed to create log directory: {}", log_dir.display()).yellow());
        eprintln!("Logs may not work until you run:");
        eprintln!("  {}", format!("sudo mkdir -p {}", log_dir.display()).cyan());
        eprintln!("  {}", format!("sudo chown -R $(whoami) {}", log_dir.display()).cyan());
        eprintln!();
        eprintln!("{}", "Continuing with installation...".yellow());
    }

    println!();
    println!("{}", "✓ Installation complete!".green().bold());
    println!();
    println!("App bundle: {}", "/Applications/Agent.app".yellow());
    println!("CLI symlink: {}", "/usr/local/bin/agent".yellow());
    println!("Services: {}", plist_dir.display().to_string().yellow());
    println!();
    println!("{}", "Usage:".cyan().bold());
    println!("  {} - Launch GUI", "agent".cyan());
    println!("  {} - List notifications", "agent notify list".cyan());
    println!("  {} - Create notification", "agent notify create --title \"Test\" --message \"Hello\"".cyan());
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

    // Stop services first
    let _ = stop_services();

    // Remove Agent.app bundle
    let app_bundle = PathBuf::from("/Applications/Agent.app");
    if app_bundle.exists() {
        fs::remove_dir_all(&app_bundle).context("Failed to remove Agent.app")?;
        println!("  {} Removed Agent.app", "✓".green());
    }

    // Remove symlink
    let prefix = get_prefix();
    let bin_dir = prefix.join("bin");
    let symlink_path = bin_dir.join("agent");
    if symlink_path.exists() {
        fs::remove_file(&symlink_path).context("Failed to remove agent symlink")?;
        println!("  {} Removed agent symlink", "✓".green());
    }

    // Remove plist files
    let plist_dir = home_dir()?.join("Library/LaunchAgents");
    let services = vec!["notify", "ask", "hook", "monitor"];

    for service in services {
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
        let plist_path = plist_dir.join(&plist_name);
        if plist_path.exists() {
            fs::remove_file(&plist_path).context(format!("Failed to remove {plist_name}"))?;
            println!("  {} Removed {}", "✓".green(), plist_name);
        }
    }

    println!();
    println!("{}", "✓ Uninstallation complete!".green().bold());

    Ok(())
}

fn start_services() -> Result<()> {
    println!("{}", "Starting services...".blue());

    let plist_dir = home_dir()?.join("Library/LaunchAgents");
    let services = vec!["notify", "ask", "hook", "monitor"];

    for service in services {
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
        let plist_path = plist_dir.join(&plist_name);

        if plist_path.exists() {
            print!("  Starting agentd-{service}... ");
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

    println!();
    println!("{}", "✓ Services started".green().bold());
    Ok(())
}

fn stop_services() -> Result<()> {
    println!("{}", "Stopping services...".blue());

    let plist_dir = home_dir()?.join("Library/LaunchAgents");
    let services = vec!["notify", "ask", "hook", "monitor"];

    for service in services {
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
        let plist_path = plist_dir.join(&plist_name);

        if plist_path.exists() {
            print!("  Stopping agentd-{service}... ");
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

    println!();
    println!("{}", "✓ Services stopped".green().bold());
    Ok(())
}

fn service_status() -> Result<()> {
    println!("{}", "Service Status:".blue().bold());
    println!();

    let output =
        Command::new("launchctl").arg("list").output().context("Failed to execute launchctl")?;

    let list_output = String::from_utf8_lossy(&output.stdout);
    let services = vec!["notify", "ask", "hook", "monitor"];

    for service in services {
        let service_name = format!("com.geoffjay.agentd-{service}");
        print!("  agentd-{service}: ");

        if list_output.contains(&service_name) {
            println!("{}", "running".green());
        } else {
            println!("{}", "stopped".red());
        }
    }

    Ok(())
}

// Helper functions

fn check_macos() -> Result<()> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("This installer is for macOS only");
    }
    Ok(())
}

fn check_in_project_root() -> Result<()> {
    if !Path::new("Cargo.toml").exists() || !Path::new("crates").exists() {
        anyhow::bail!("Must be run from the agentd project root");
    }
    Ok(())
}

fn build_release() -> Result<()> {
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

fn install_binaries(bin_dir: &Path) -> Result<()> {
    println!("{}", "Installing Agent.app bundle...".blue());

    // Create Agent.app bundle structure
    let app_bundle = PathBuf::from("/Applications/Agent.app");
    let macos_dir = app_bundle.join("Contents/MacOS");
    let resources_dir = app_bundle.join("Contents/Resources");

    fs::create_dir_all(&macos_dir).context("Failed to create Agent.app/Contents/MacOS")?;
    fs::create_dir_all(&resources_dir).context("Failed to create Agent.app/Contents/Resources")?;

    // Copy Info.plist
    let info_plist_src = Path::new("crates/ui/Info.plist");
    let info_plist_dest = app_bundle.join("Contents/Info.plist");
    if info_plist_src.exists() {
        fs::copy(info_plist_src, info_plist_dest).context("Failed to copy Info.plist")?;
        println!("  {} Info.plist", "✓".green());
    }

    // Install GUI binary to Agent.app/Contents/MacOS/agent
    let gui_src = Path::new("target/release/agent");
    let gui_dest = macos_dir.join("agent");
    if gui_src.exists() {
        fs::copy(gui_src, &gui_dest).context("Failed to install GUI binary")?;
        set_executable(&gui_dest)?;
        println!("  {} GUI binary (agent)", "✓".green());
    } else {
        println!("  {} GUI binary (not built yet)", "⚠".yellow());
    }

    // Install CLI binary to Agent.app/Contents/MacOS/cli
    let cli_src = Path::new("target/release/cli");
    let cli_dest = macos_dir.join("cli");
    if cli_src.exists() {
        fs::copy(cli_src, &cli_dest).context("Failed to install CLI binary")?;
        set_executable(&cli_dest)?;
        println!("  {} CLI binary (cli)", "✓".green());
    } else {
        println!("  {} CLI binary (not built)", "⚠".yellow());
    }

    // Install service binaries to Agent.app/Contents/MacOS/
    let services = vec![
        ("agentd-notify", "target/release/agentd-notify"),
        ("agentd-ask", "target/release/agentd-ask"),
        ("agentd-hook", "target/release/agentd-hook"),
        ("agentd-monitor", "target/release/agentd-monitor"),
    ];

    for (name, src_path) in services {
        let src = Path::new(src_path);
        let dest = macos_dir.join(name);

        if src.exists() {
            fs::copy(src, &dest).context(format!("Failed to install {name}"))?;
            set_executable(&dest)?;
            println!("  {} {}", "✓".green(), name);
        } else {
            println!("  {} {} (not built)", "⚠".yellow(), name);
        }
    }

    // Create symlink from /usr/local/bin/agent to CLI
    println!();
    println!("{}", "Creating symlink...".blue());

    fs::create_dir_all(bin_dir).context("Failed to create /usr/local/bin")?;

    let symlink_path = bin_dir.join("agent");
    let target_path = macos_dir.join("cli");

    // Remove existing symlink if present
    if symlink_path.exists() {
        fs::remove_file(&symlink_path).ok();
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if let Err(e) = symlink(&target_path, &symlink_path) {
            eprintln!("{}", format!("Failed to create symlink: {}", e).red());
            eprintln!("{}", "To fix permissions, run:".yellow());
            eprintln!("  {}", format!("sudo chown -R $(whoami) {}", bin_dir.display()).cyan());
            return Err(e.into());
        }
    }

    println!("  {} agent -> {}", "✓".green(), target_path.display());

    Ok(())
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

fn install_plists(plist_dir: &Path) -> Result<()> {
    println!("{}", "Installing service plists...".blue());

    let services = vec!["notify", "ask", "hook", "monitor"];
    let plist_src_dir = Path::new("contrib/plists");

    for service in services {
        let plist_name = format!("com.geoffjay.agentd-{service}.plist");
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

fn get_prefix() -> PathBuf {
    env::var("PREFIX").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/usr/local"))
}

fn home_dir() -> Result<PathBuf> {
    env::var("HOME").map(PathBuf::from).context("HOME environment variable not set")
}
