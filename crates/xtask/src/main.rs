//! agentd xtask — Build and installation automation (developer-only).
//!
//! The reusable install / service-management / migration logic lives in the
//! `agentd-install` crate so the shipped `agent` binary can run it on
//! production hosts. This crate is the *developer* front-end: it builds the
//! binaries and UI from the source tree first, then delegates the platform
//! setup to `agentd-install`.
//!
//! # Commands
//!
//! - `install-user` / `install` — Build everything and install for the current user
//!   (set `AGENTD_PAM=1` to build `agentd-core` with PAM system-user login support)
//! - `uninstall` — Remove all installed components
//! - `start-services` / `stop-services` / `restart-services` — Service lifecycle
//! - `start-service` / `stop-service` / `restart-service` <name> — Single-service lifecycle
//! - `service-status` — Check running state of all services
//! - `generate-entities [--service <name>]` — Regenerate SeaORM entity files via sea-orm-cli
//! - `migrate [--service <name>]` — Apply pending SeaORM migrations
//! - `migrate-status [--service <name>]` — Show migration status for all databases
//! - `release [--dry-run]` — Tag the current version and prepare a release

use agentd_install::{detect_platform, get_prefix, home_dir, validate_service_name, InstallPaths};
use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let task = args.get(1).map(|s| s.as_str());

    match task {
        Some("install") | Some("install-user") => install_user()?,
        Some("install-completions") => install_completions()?,
        Some("uninstall") => uninstall()?,
        Some("start-services") => with_summary("Starting services...", "Services started", || {
            detect_platform().start_services()
        })?,
        Some("stop-services") => with_summary("Stopping services...", "Services stopped", || {
            detect_platform().stop_services()
        })?,
        Some("restart-services") => restart_services()?,
        Some("service-status") => service_status()?,
        Some("start-service") => {
            let service = args.get(2).context("Service name required")?;
            validate_service_name(service)?;
            detect_platform().start_service(service)?;
        }
        Some("stop-service") => {
            let service = args.get(2).context("Service name required")?;
            validate_service_name(service)?;
            detect_platform().stop_service(service)?;
        }
        Some("restart-service") => {
            let service = args.get(2).context("Service name required")?;
            restart_service(service)?;
        }
        Some("release") => {
            let dry_run = args.iter().any(|a| a == "--dry-run");
            release(dry_run)?;
        }
        Some("generate-entities") => {
            let service = parse_service_flag(&args);
            generate_entities(service.as_deref())?;
        }
        Some("migrate") => {
            let service = parse_service_flag(&args);
            tokio::runtime::Runtime::new()?
                .block_on(agentd_install::migrate::migrate(service.as_deref()))?;
        }
        Some("migrate-status") => {
            let service = parse_service_flag(&args);
            tokio::runtime::Runtime::new()?
                .block_on(agentd_install::migrate::migrate_status(service.as_deref()))?;
        }
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    println!("{}", "agentd xtask commands:".blue().bold());
    println!();
    println!("{}", "Installation:".cyan());
    println!("  {} - Build & install for current user", "install-user".green());
    println!(
        "      {} build agentd-core with PAM (system-user login) support",
        "AGENTD_PAM=1".yellow()
    );
    println!("  {} - Generate & install shell completions", "install-completions".green());
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
    println!("{}", "Database:".cyan());
    println!(
        "  {} [--service <name>] - Regenerate SeaORM entity files from database schema",
        "generate-entities".green()
    );
    println!("  {} [--service <name>] - Apply pending SeaORM migrations", "migrate".green());
    println!(
        "  {} [--service <name>] - Show migration status for all databases",
        "migrate-status".green()
    );
    println!();
    println!("{}", "Release:".cyan());
    println!("  {} [--dry-run] - Tag the current version and prepare a release", "release".green());
    println!();
    println!("{}", "Examples:".cyan());
    println!("  {}", "cargo xtask install-user".yellow());
    println!("  {}", "cargo xtask start-service notify".yellow());
    println!("  {}", "cargo xtask restart-service ask".yellow());
    println!("  {}", "cargo xtask migrate".yellow());
    println!("  {}", "cargo xtask migrate --service notify".yellow());
    println!("  {}", "cargo xtask migrate-status".yellow());
    println!("  {}", "cargo xtask generate-entities".yellow());
    println!("  {}", "cargo xtask generate-entities --service orchestrator".yellow());
    println!("  {}", "cargo xtask release --dry-run".yellow());
    println!("  {}", "cargo xtask release".yellow());
    println!();
    println!("{}", "Available services:".cyan());
    println!("  {}", agentd_install::SERVICE_NAMES.join(", "));
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

// ---------------------------------------------------------------------------
// Database: entity generation (dev-only; requires sea-orm-cli)
// ---------------------------------------------------------------------------

/// Services that have SeaORM-managed SQLite databases, with their entity output
/// directories (used only by `generate-entities`).
const DB_SERVICES: &[DbService] = &[
    DbService {
        name: "communicate",
        project: "agentd-communicate",
        db_file: "communicate.db",
        entity_dir: "crates/communicate/src/entity",
    },
    DbService {
        name: "core",
        project: "agentd-core",
        db_file: "core.db",
        entity_dir: "crates/core/src/entity",
    },
    DbService {
        name: "knowledge",
        project: "agentd-knowledge",
        db_file: "knowledge.db",
        entity_dir: "crates/knowledge/src/entity",
    },
    DbService {
        name: "memory",
        project: "agentd-memory",
        db_file: "memory.db",
        entity_dir: "crates/memory/src/entity",
    },
    DbService {
        name: "notify",
        project: "agentd-notify",
        db_file: "notify.db",
        entity_dir: "crates/notify/src/entity",
    },
    DbService {
        name: "orchestrator",
        project: "agentd-orchestrator",
        db_file: "orchestrator.db",
        entity_dir: "crates/orchestrator/src/entity",
    },
];

struct DbService {
    name: &'static str,
    project: &'static str,
    db_file: &'static str,
    entity_dir: &'static str,
}

/// Parse `--service <name>` from the argument list, returning the service name if present.
fn parse_service_flag(args: &[String]) -> Option<String> {
    args.windows(2).find(|w| w[0] == "--service").map(|w| w[1].clone())
}

fn resolve_entity_services(service: Option<&str>) -> Result<Vec<&'static DbService>> {
    match service {
        Some(name) => {
            let svc = DB_SERVICES.iter().find(|s| s.name == name).with_context(|| {
                format!(
                    "Unknown service '{name}'. Valid: {}",
                    DB_SERVICES.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
                )
            })?;
            Ok(vec![svc])
        }
        None => Ok(DB_SERVICES.iter().collect()),
    }
}

/// `cargo xtask generate-entities [--service <name>]`
///
/// Runs `sea-orm-cli generate entity` for each (or the specified) service with
/// a SeaORM-managed SQLite database. Requires `sea-orm-cli` to be installed.
fn generate_entities(service: Option<&str>) -> Result<()> {
    check_in_project_root()?;

    if Command::new("sea-orm-cli").arg("--version").output().is_err() {
        eprintln!("{}", "sea-orm-cli not found.".red().bold());
        eprintln!("Install it with: {}", "cargo install sea-orm-cli".cyan());
        anyhow::bail!("sea-orm-cli is required for entity generation");
    }

    let services = resolve_entity_services(service)?;

    println!("{}", "Generating SeaORM entities...".blue().bold());
    println!();

    for svc in services {
        let db_path = agentd_common::storage::get_db_path(svc.project, svc.db_file)?;

        if !db_path.exists() {
            eprintln!(
                "  {} {} — database not found at {}",
                "⚠".yellow(),
                svc.name.yellow(),
                db_path.display()
            );
            eprintln!(
                "  {} Start the service once to create the database, then re-run.",
                "hint:".bright_black()
            );
            continue;
        }

        let db_url = format!("sqlite://{}", db_path.display());
        println!("  {} {}  ({})", "→".cyan(), svc.name.green(), db_path.display());

        let status = Command::new("sea-orm-cli")
            .args([
                "generate",
                "entity",
                "--database-url",
                &db_url,
                "--output-dir",
                svc.entity_dir,
                "--with-serde",
                "both",
            ])
            .status()
            .with_context(|| format!("Failed to run sea-orm-cli for {}", svc.name))?;

        if status.success() {
            println!("  {} {} entities written to {}", "✓".green(), svc.name, svc.entity_dir);
        } else {
            eprintln!("  {} sea-orm-cli failed for {}", "✗".red(), svc.name);
        }
    }

    println!();
    println!("{}", "Entity generation complete.".green().bold());
    println!();
    println!(
        "{}",
        "Note: Generated files are a scaffold — review before committing.".bright_black()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Installation (dev): build from source, then delegate platform setup
// ---------------------------------------------------------------------------

fn install_user() -> Result<()> {
    let mode = if agentd_install::is_root() { "system" } else { "user" };
    println!("{}", format!("Installing agentd ({mode} mode)...").blue().bold());
    println!();

    check_in_project_root()?;

    // Build binaries
    println!("{}", "Building binaries...".blue());
    build_release()?;

    // Build UI
    println!();
    println!("{}", "Building UI...".blue());
    build_ui()?;

    // Determine install prefix and bin directory
    let prefix = get_prefix();
    let bin_dir = prefix.join("bin");

    if let Err(e) = fs::create_dir_all(&bin_dir) {
        eprintln!("{}", format!("Failed to create directory: {}", bin_dir.display()).red());
        eprintln!("{}", "To fix permissions, run:".yellow());
        eprintln!("  {}", format!("sudo mkdir -p {}", bin_dir.display()).cyan());
        eprintln!("  {}", format!("sudo chown -R $(whoami) {}", prefix.display()).cyan());
        return Err(e.into());
    }

    // Delegate to the shared platform installer, sourcing the freshly-built
    // binaries from target/release and the UI from ui/dist.
    let plat = detect_platform();
    plat.install(&InstallPaths {
        bin_src: Path::new("target/release"),
        bin_dir: &bin_dir,
        ui_src: Some(Path::new("ui/dist")),
    })?;

    // Install shell completions
    println!();
    if let Err(e) = install_completions() {
        eprintln!("{}", format!("Warning: Failed to install shell completions: {}", e).yellow());
    }

    println!();
    println!("{}", "✓ Installation complete!".green().bold());
    println!();
    plat.print_install_summary()?;
    println!();
    println!("{}", "Usage:".cyan().bold());
    println!("  {} - List notifications", "agent notify list".cyan());
    println!();
    println!("To start services: {}", "cargo xtask start-services".cyan());

    Ok(())
}

fn uninstall() -> Result<()> {
    println!("{}", "Uninstalling agentd...".blue().bold());
    detect_platform().uninstall()?;
    println!();
    println!("{}", "✓ Uninstallation complete!".green().bold());
    Ok(())
}

fn with_summary<F>(start_msg: &str, done_msg: &str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    println!("{}", start_msg.blue());
    f()?;
    println!();
    println!("{}", format!("✓ {done_msg}").green().bold());
    Ok(())
}

fn restart_services() -> Result<()> {
    println!("{}", "Restarting all services...".blue());
    println!();
    let plat = detect_platform();
    plat.stop_services()?;
    println!();
    plat.start_services()?;
    Ok(())
}

fn restart_service(service: &str) -> Result<()> {
    validate_service_name(service)?;
    println!("{}", format!("Restarting agentd-{service}...").blue());
    let plat = detect_platform();
    plat.stop_service(service)?;
    plat.start_service(service)?;
    println!();
    println!("{}", format!("✓ Service agentd-{service} restarted").green().bold());
    Ok(())
}

fn service_status() -> Result<()> {
    println!("{}", "Service Status:".blue().bold());
    println!();
    detect_platform().service_status()
}

fn install_completions() -> Result<()> {
    println!("{}", "Installing shell completions...".blue().bold());
    println!();

    let bin_dir = get_prefix().join("bin");
    let agent_bin = bin_dir.join("agent");

    // Prefer the installed binary; otherwise fall back to the release build.
    let agent_cmd = if agent_bin.exists() {
        agent_bin.to_string_lossy().to_string()
    } else {
        let release_bin = Path::new("target/release/cli");
        if release_bin.exists() {
            release_bin.to_string_lossy().to_string()
        } else {
            println!("{}", "agent binary not found. Building...".yellow());
            build_release()?;
            "target/release/cli".to_string()
        }
    };

    let home = home_dir()?;

    // (shell, target dir, file name)
    let targets = [
        ("bash", home.join(".local/share/bash-completion/completions"), "agent"),
        ("zsh", home.join(".zfunc"), "_agent"),
        ("fish", home.join(".config/fish/completions"), "agent.fish"),
    ];

    for (shell, dir, file) in targets {
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("  {} {}: {}", "⚠".yellow(), shell, e);
            continue;
        }
        let output = Command::new(&agent_cmd)
            .args(["completions", shell])
            .output()
            .with_context(|| format!("Failed to generate {shell} completions"))?;
        if output.status.success() {
            fs::write(dir.join(file), &output.stdout)?;
            println!("  {} {} → {}", "✓".green(), shell, dir.join(file).display());
        }
    }

    println!();
    println!("{}", "✓ Shell completions installed!".green().bold());
    println!();
    println!(
        "{}",
        "Note: You may need to restart your shell or source the completions.".bright_black()
    );
    println!(
        "{}",
        "For zsh, ensure ~/.zfunc is in your fpath: fpath=(~/.zfunc $fpath)".bright_black()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Release
// ---------------------------------------------------------------------------

/// `cargo xtask release [--dry-run]`
///
/// Prepares a release for the current workspace version:
/// 1. Verifies the working tree is clean.
/// 2. Reads `[workspace.package] version` from the root `Cargo.toml`.
/// 3. Optionally updates `CHANGELOG.md` via `git-cliff` (if installed).
/// 4. Creates an annotated git tag `v{version}`.
fn release(dry_run: bool) -> Result<()> {
    check_in_project_root()?;

    if !dry_run {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .output()
            .context("Failed to run git status")?;
        let dirty = String::from_utf8_lossy(&output.stdout);
        if !dirty.trim().is_empty() {
            anyhow::bail!(
                "Working tree has uncommitted changes. Commit or stash them before releasing.\n{}",
                dirty.trim()
            );
        }
    }

    let cargo_toml_src =
        fs::read_to_string("Cargo.toml").context("Failed to read workspace Cargo.toml")?;
    let version = extract_workspace_version(&cargo_toml_src)?;
    let tag = format!("v{version}");

    println!("{}", "Release Preparation".blue().bold());
    println!();
    println!("  Version : {}", version.cyan());
    println!("  Tag     : {}", tag.cyan());
    if dry_run {
        println!();
        println!("{}", "(dry-run mode — no changes will be made)".yellow());
    }

    let has_cliff = Command::new("git-cliff").arg("--version").output().is_ok();
    if has_cliff {
        println!();
        if dry_run {
            println!("{}", "Would run: git cliff --current --output CHANGELOG.md".bright_black());
        } else {
            println!("{}", "Updating CHANGELOG.md via git-cliff...".blue());
            let status = Command::new("git-cliff")
                .args(["--current", "--output", "CHANGELOG.md"])
                .status()
                .context("Failed to run git-cliff")?;
            if status.success() {
                println!("  {} CHANGELOG.md updated", "✓".green());
                let changelog_changed = Command::new("git")
                    .args(["diff", "--quiet", "CHANGELOG.md"])
                    .status()
                    .context("Failed to check CHANGELOG.md diff")?;
                if !changelog_changed.success() {
                    let add_status = Command::new("git")
                        .args(["add", "CHANGELOG.md"])
                        .status()
                        .context("Failed to stage CHANGELOG.md")?;
                    if !add_status.success() {
                        anyhow::bail!("git add CHANGELOG.md failed");
                    }
                    let commit_status = Command::new("git")
                        .args([
                            "commit",
                            "-m",
                            &format!("chore(release): update changelog for {tag}"),
                        ])
                        .status()
                        .context("Failed to commit CHANGELOG.md")?;
                    if !commit_status.success() {
                        anyhow::bail!("git commit for changelog update failed");
                    }
                    println!("  {} changelog commit created", "✓".green());
                }
            } else {
                eprintln!("  {} git-cliff failed — CHANGELOG.md not updated", "⚠".yellow());
            }
        }
    } else {
        println!();
        println!("  {} git-cliff not found — skipping changelog update", "⚠".yellow());
        println!("  Install: {}", "cargo install git-cliff".cyan());
    }

    if dry_run {
        println!();
        println!("{}", format!("Would create annotated tag: {tag}").yellow());
        println!("{}", "Run without --dry-run to proceed.".bright_black());
        return Ok(());
    }

    println!();
    println!("{}", format!("Creating tag {tag}...").blue());
    let tag_status = Command::new("git")
        .args(["tag", "-a", &tag, "-m", &format!("Release {tag}")])
        .status()
        .context("Failed to create git tag")?;
    if !tag_status.success() {
        anyhow::bail!("Failed to create git tag '{tag}' — does it already exist?");
    }
    println!("  {} tag {} created", "✓".green(), tag.cyan());

    println!();
    println!("{}", "✓ Release prepared!".green().bold());
    println!();
    println!("Push the tag to trigger the GitHub Actions release workflow:");
    println!("  {}", format!("git push origin {tag}").cyan());

    Ok(())
}

/// Extract `version` from the `[workspace.package]` table in a `Cargo.toml` string.
fn extract_workspace_version(cargo_toml: &str) -> Result<String> {
    let mut in_section = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.starts_with('[') {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("version") {
                if let Some(rest) = rest.trim().strip_prefix('=') {
                    let ver = rest.trim().trim_matches('"');
                    return Ok(ver.to_string());
                }
            }
        }
    }
    anyhow::bail!("Could not find `version` in `[workspace.package]` section of Cargo.toml")
}

// ---------------------------------------------------------------------------
// Build helpers
// ---------------------------------------------------------------------------

fn check_in_project_root() -> Result<()> {
    if !Path::new("Cargo.toml").exists() || !Path::new("crates").exists() {
        anyhow::bail!("Must be run from the agentd project root");
    }
    Ok(())
}

/// Whether to build `agentd-core` with PAM (system-user login) support.
///
/// Opt-in via `AGENTD_PAM=1` (also accepts `true`/`yes`/`on`). The `pam` feature
/// links the system PAM library and is off by default so standard installs don't
/// need it; this gate lets `install-user` produce a PAM-enabled build.
fn pam_enabled() -> bool {
    std::env::var("AGENTD_PAM")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
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

    // Opt-in: rebuild agentd-core with PAM support. This second build overwrites
    // target/release/agentd-core with the feature-enabled binary the installer
    // then sources. Done as a separate `-p agentd-core` build (rather than a
    // workspace `--features`) since the `pam` feature only exists on that crate.
    if pam_enabled() {
        println!("{}", "Rebuilding agentd-core with PAM support (AGENTD_PAM set)...".blue());
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("-p")
            .arg("agentd-core")
            .arg("--features")
            .arg("pam")
            .status()
            .context("Failed to execute PAM-enabled cargo build")?;

        if !status.success() {
            anyhow::bail!(
                "PAM-enabled build failed. On Linux, ensure the PAM dev library is installed \
                 (libpam0g-dev on Debian/Ubuntu, pam-devel on RHEL/Fedora); macOS needs no extra packages."
            );
        }
    }

    Ok(())
}

/// Build the UI by running `bun install` and `bun run build` in the `ui/` directory.
fn build_ui() -> Result<()> {
    let ui_dir = Path::new("ui");
    if !ui_dir.exists() {
        anyhow::bail!("ui/ directory not found — must be run from project root");
    }

    if Command::new("bun").arg("--version").output().is_err() {
        eprintln!("{}", "bun not found.".red().bold());
        eprintln!("Install bun: {}", "https://bun.sh".cyan());
        anyhow::bail!("bun is required to build the UI");
    }

    println!("  Running bun install...");
    let install_status = Command::new("bun")
        .arg("install")
        .current_dir(ui_dir)
        .status()
        .context("Failed to execute bun install")?;

    if !install_status.success() {
        anyhow::bail!("bun install failed");
    }

    println!("  Running bun run build...");
    let build_status = Command::new("bun")
        .arg("run")
        .arg("build")
        .current_dir(ui_dir)
        .env("NODE_ENV", "production")
        .status()
        .context("Failed to execute bun run build")?;

    if !build_status.success() {
        anyhow::bail!("bun run build failed");
    }

    let dist_dir = ui_dir.join("dist");
    if !dist_dir.exists() {
        anyhow::bail!("UI build completed but ui/dist/ was not created");
    }

    println!("  {} UI built to ui/dist/", "✓".green());
    Ok(())
}
