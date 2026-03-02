use anyhow::{Context, Result};
use colored::Colorize;
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
        Some("hex-to-rgb-hsl") => {
            let hex = args.get(2).context("Hex color value required (e.g., dc8a78 or #dc8a78)")?;
            hex_to_rgb_hsl(hex)?;
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
    println!("{}", "Development Utilities:".cyan());
    println!("  {} <hex> - Convert hex color to RGB and HSL", "hex-to-rgb-hsl".green());
    println!();
    println!("{}", "Examples:".cyan());
    println!("  {}", "cargo xtask install-user".yellow());
    println!("  {}", "cargo xtask start-service notify".yellow());
    println!("  {}", "cargo xtask restart-service ask".yellow());
    println!("  {}", "cargo xtask hex-to-rgb-hsl dc8a78".yellow());
    println!();
    println!("{}", "Available services:".cyan());
    println!("  notify, ask, hook, monitor, wrap, orchestrator");
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

    // Create and setup log directory
    println!();
    println!("{}", "Setting up log directory...".blue());
    let log_dir = prefix.join("var/log");

    // Ensure directory exists
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

    // Always fix ownership, not just when creating
    let user = env::var("USER")
        .unwrap_or_else(|_| env::var("LOGNAME").unwrap_or_else(|_| "user".to_string()));

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

    println!();
    println!("{}", "✓ Installation complete!".green().bold());
    println!();
    println!("App bundle: {}", "/Applications/Agent.app".yellow());
    println!("CLI symlink: {}", "/usr/local/bin/agent".yellow());
    println!("Services: {}", plist_dir.display().to_string().yellow());
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
    let services = vec!["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];

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
    let services = vec!["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];

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
    let services = vec!["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];

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
    let services = vec!["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];

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

fn start_service(service: &str) -> Result<()> {
    validate_service_name(service)?;

    let plist_dir = home_dir()?.join("Library/LaunchAgents");
    let plist_name = format!("com.geoffjay.agentd-{service}.plist");
    let plist_path = plist_dir.join(&plist_name);

    if !plist_path.exists() {
        anyhow::bail!("Service '{service}' not installed. Run 'cargo xtask install-user' first.");
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

fn stop_service(service: &str) -> Result<()> {
    validate_service_name(service)?;

    let plist_dir = home_dir()?.join("Library/LaunchAgents");
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

    stop_service(service)?;
    start_service(service)?;

    println!();
    println!("{}", format!("✓ Service agentd-{service} restarted").green().bold());

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
        ("agentd-wrap", "target/release/agentd-wrap"),
        ("agentd-orchestrator", "target/release/agentd-orchestrator"),
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

    let symlink_path = bin_dir.join("agent");
    let target_path = macos_dir.join("cli");

    // Try to create the symlink, use sudo if needed
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        // First, try creating the directory if needed
        let needs_sudo =
            if !bin_dir.exists() { fs::create_dir_all(bin_dir).is_err() } else { false };

        // Remove existing symlink if present (might need sudo)
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

        // Try to create symlink without sudo first
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
                // Permission denied - use sudo for just the symlink
                println!("{}", "  Creating symlink requires sudo...".yellow());

                // Use sudo to create the symlink
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

    let services = vec!["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];
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

fn validate_service_name(service: &str) -> Result<()> {
    let valid_services = ["notify", "ask", "hook", "monitor", "wrap", "orchestrator"];
    if !valid_services.contains(&service) {
        anyhow::bail!(
            "Invalid service name: '{}'. Valid services are: {}",
            service,
            valid_services.join(", ")
        );
    }
    Ok(())
}

/// Convert a hex color string to RGB and HSL formats.
///
/// Accepts hex colors with or without the # prefix (e.g., "dc8a78" or "#dc8a78").
/// Outputs JSON with RGB (0-255) and HSL (h: 0-360, s: 0-1, l: 0-1) values.
///
/// # Arguments
///
/// * `hex` - Hex color string (3 or 6 digits, with or without # prefix)
///
/// # Examples
///
/// ```bash
/// cargo xtask hex-to-rgb-hsl dc8a78
/// cargo xtask hex-to-rgb-hsl "#dc8a78"
/// cargo xtask hex-to-rgb-hsl fff
/// ```
fn hex_to_rgb_hsl(hex: &str) -> Result<()> {
    // Strip # prefix if present
    let hex = hex.strip_prefix('#').unwrap_or(hex);

    // Expand 3-digit hex to 6-digit
    let hex = if hex.len() == 3 {
        format!(
            "{}{}{}{}{}{}",
            &hex[0..1],
            &hex[0..1],
            &hex[1..2],
            &hex[1..2],
            &hex[2..3],
            &hex[2..3]
        )
    } else if hex.len() == 6 {
        hex.to_string()
    } else {
        anyhow::bail!("Invalid hex color: must be 3 or 6 digits (got {})", hex.len());
    };

    // Parse RGB components
    let r = u8::from_str_radix(&hex[0..2], 16)
        .context("Invalid hex color: failed to parse red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .context("Invalid hex color: failed to parse green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .context("Invalid hex color: failed to parse blue component")?;

    // Convert RGB to HSL
    let (h, s, l) = rgb_to_hsl(r, g, b);

    // Output as JSON
    let output = serde_json::json!({
        "rgb": {
            "r": r,
            "g": g,
            "b": b
        },
        "hsl": {
            "h": h,
            "s": s,
            "l": l
        }
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

/// Convert RGB values (0-255) to HSL (h: 0-360, s: 0-1, l: 0-1).
///
/// Implements the standard RGB to HSL conversion algorithm.
///
/// # Arguments
///
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Returns
///
/// Returns a tuple (h, s, l) where:
/// - h: Hue in degrees (0-360)
/// - s: Saturation (0-1)
/// - l: Lightness (0-1)
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    // Normalize RGB values to 0-1 range
    let r = f64::from(r) / 255.0;
    let g = f64::from(g) / 255.0;
    let b = f64::from(b) / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Calculate lightness
    let l = (max + min) / 2.0;

    // Calculate saturation
    let s = if delta == 0.0 { 0.0 } else { delta / (1.0 - (2.0 * l - 1.0).abs()) };

    // Calculate hue
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    // Normalize hue to 0-360 range
    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, l)
}
