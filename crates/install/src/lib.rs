//! Installation and service-management logic for agentd.
//!
//! This crate contains the environment-agnostic logic for installing agentd
//! binaries, writing platform service definitions (launchd plists on macOS,
//! systemd units on Linux), gap-filling the agentd config file, and running
//! SeaORM migrations.
//!
//! It is consumed by two callers:
//! - the shipped `agent` CLI (`agent install`, `agent service …`, `agent migrate`),
//!   which runs on production hosts with no source tree and no `cargo`/`bun`;
//! - the dev-only `xtask` crate, which builds binaries and the UI first, then
//!   delegates the platform setup here.
//!
//! The only inputs that differ between dev and production are the *source*
//! directories for binaries and UI assets, which the caller supplies via
//! [`InstallPaths`].

pub mod install_config;
pub mod migrate;
pub mod platform;

pub use platform::{detect_platform, Platform, ServiceInfo, SERVICES, SERVICE_NAMES};

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Source and destination paths for an install.
///
/// `bin_src` and `ui_src` are the only values that differ between the dev path
/// (where binaries are freshly built into `target/release` and the UI into
/// `ui/dist`) and the production path (where the installer has already placed
/// the downloaded binaries on disk).
pub struct InstallPaths<'a> {
    /// Directory containing the built/downloaded binaries (`agent`/`cli` and
    /// the `agentd-*` services).
    pub bin_src: &'a Path,
    /// Final user-facing `bin` directory (used for the `agent` symlink on
    /// macOS and as the binary location on Linux).
    pub bin_dir: &'a Path,
    /// Directory containing the built UI assets (e.g. `ui/dist`), or `None` to
    /// skip UI installation.
    pub ui_src: Option<&'a Path>,
}

/// Make a file executable (0o755) on Unix. No-op elsewhere.
pub fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// Resolve the install prefix.
///
/// Honours `$PREFIX`; otherwise `/usr/local` on macOS or when running as root,
/// and `~/.local` for an unprivileged Linux install.
pub fn get_prefix() -> PathBuf {
    env::var("PREFIX").map(PathBuf::from).unwrap_or_else(|_| {
        if cfg!(target_os = "macos") || is_root() {
            PathBuf::from("/usr/local")
        } else {
            home_dir().unwrap_or_else(|_| PathBuf::from("/usr/local")).join(".local")
        }
    })
}

/// Returns true when the effective user ID is 0 (root).
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|uid| uid == 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Resolve `$HOME`.
pub fn home_dir() -> Result<PathBuf> {
    env::var("HOME").map(PathBuf::from).context("HOME environment variable not set")
}

/// Validate that `service` is a known service name.
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
