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

pub use platform::{
    detect_platform, detect_platform_for, Platform, ServiceInfo, SERVICES, SERVICE_NAMES,
};

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

/// Whether to install for the current user or system-wide.
///
/// This is the explicit form of the historical, privilege-derived behaviour:
/// `--user` maps to [`InstallScope::User`], `--system` to
/// [`InstallScope::System`], and the absence of either flag to
/// [`InstallScope::Auto`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InstallScope {
    /// Decide automatically: system-wide on macOS or when running as root,
    /// per-user otherwise. Preserves the historical default.
    #[default]
    Auto,
    /// Per-user install (`~/.local`; systemd `--user` units on Linux).
    User,
    /// System-wide install (`/usr/local`; system systemd units on Linux,
    /// which requires root).
    System,
}

impl InstallScope {
    /// Resolve to a concrete system-vs-user decision.
    ///
    /// [`InstallScope::Auto`] mirrors the legacy logic: system-wide on macOS or
    /// when running as root, per-user otherwise.
    pub fn is_system(self) -> bool {
        match self {
            InstallScope::User => false,
            InstallScope::System => true,
            InstallScope::Auto => cfg!(target_os = "macos") || is_root(),
        }
    }
}

/// Resolve the install prefix for an explicit [`InstallScope`].
///
/// Honours `$PREFIX`; otherwise `/usr/local` for a system install and
/// `~/.local` for a per-user install.
pub fn get_prefix_for(scope: InstallScope) -> PathBuf {
    env::var("PREFIX").map(PathBuf::from).unwrap_or_else(|_| {
        if scope.is_system() {
            PathBuf::from("/usr/local")
        } else {
            home_dir().unwrap_or_else(|_| PathBuf::from("/usr/local")).join(".local")
        }
    })
}

/// Resolve the install prefix using the automatic scope.
///
/// Honours `$PREFIX`; otherwise `/usr/local` on macOS or when running as root,
/// and `~/.local` for an unprivileged Linux install.
pub fn get_prefix() -> PathBuf {
    get_prefix_for(InstallScope::Auto)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_scope_ignores_privileges_and_platform() {
        assert!(!InstallScope::User.is_system());
        assert!(InstallScope::System.is_system());
    }

    #[test]
    fn auto_scope_is_system_on_macos_or_root() {
        let expected = cfg!(target_os = "macos") || is_root();
        assert_eq!(InstallScope::Auto.is_system(), expected);
    }

    #[test]
    fn get_prefix_for_user_is_under_home() {
        // $PREFIX takes precedence and would mask the scope-based branch.
        if env::var_os("PREFIX").is_some() {
            return;
        }
        let prefix = get_prefix_for(InstallScope::User);
        assert!(prefix.ends_with(".local"), "user prefix should be ~/.local, got {prefix:?}");
    }

    #[test]
    fn get_prefix_for_system_is_usr_local() {
        if env::var_os("PREFIX").is_some() {
            return;
        }
        assert_eq!(get_prefix_for(InstallScope::System), PathBuf::from("/usr/local"));
    }
}
