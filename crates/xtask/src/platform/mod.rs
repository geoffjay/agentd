//! Platform abstraction for service management.
//!
//! This module provides a trait-based abstraction over platform-specific service
//! management. Each platform (macOS, Linux) implements the [`Platform`] trait to
//! handle installation, service lifecycle, and status checking using the native
//! service manager (launchd on macOS, systemd on Linux).

pub mod linux;
pub mod macos;

use anyhow::Result;
use std::path::Path;

/// Metadata for a managed service.
pub struct ServiceInfo {
    /// Short name used in CLI commands (e.g. "notify").
    pub name: &'static str,
    /// Binary name (e.g. "agentd-notify").
    pub binary: &'static str,
    /// Production port number.
    pub port: u16,
    /// Additional environment variables beyond RUST_LOG and PORT.
    pub extra_env: &'static [(&'static str, &'static str)],
}

/// Canonical list of all managed services.
pub const SERVICES: &[ServiceInfo] = &[
    ServiceInfo {
        name: "ask",
        binary: "agentd-ask",
        port: 7001,
        extra_env: &[("NOTIFY_SERVICE_URL", "http://localhost:7004")],
    },
    ServiceInfo { name: "hook", binary: "agentd-hook", port: 7002, extra_env: &[] },
    ServiceInfo { name: "monitor", binary: "agentd-monitor", port: 7003, extra_env: &[] },
    ServiceInfo { name: "notify", binary: "agentd-notify", port: 7004, extra_env: &[] },
    ServiceInfo { name: "wrap", binary: "agentd-wrap", port: 7005, extra_env: &[] },
    ServiceInfo { name: "orchestrator", binary: "agentd-orchestrator", port: 7006, extra_env: &[] },
];

/// All valid service names.
pub const SERVICE_NAMES: &[&str] = &["ask", "hook", "monitor", "notify", "wrap", "orchestrator"];

/// Look up a service by short name.
#[cfg(test)]
pub fn get_service_info(name: &str) -> Option<&'static ServiceInfo> {
    SERVICES.iter().find(|s| s.name == name)
}

/// Platform-specific service management operations.
pub trait Platform {
    /// Install binaries and service configuration files.
    fn install(&self, bin_dir: &Path) -> Result<()>;

    /// Remove all installed components.
    fn uninstall(&self) -> Result<()>;

    /// Start all installed services.
    fn start_services(&self) -> Result<()>;

    /// Stop all running services.
    fn stop_services(&self) -> Result<()>;

    /// Start a single service by name.
    fn start_service(&self, service: &str) -> Result<()>;

    /// Stop a single service by name.
    fn stop_service(&self, service: &str) -> Result<()>;

    /// Print status of all services.
    fn service_status(&self) -> Result<()>;

    /// Display platform-specific post-install summary.
    fn print_install_summary(&self) -> Result<()>;
}

/// Detect the current platform and return the appropriate implementation.
pub fn detect_platform() -> Box<dyn Platform> {
    if cfg!(target_os = "macos") {
        Box::new(macos::MacOSPlatform)
    } else if cfg!(target_os = "linux") {
        Box::new(linux::LinuxPlatform)
    } else {
        // Fall back to Linux-style for other Unix systems
        Box::new(linux::LinuxPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_info_completeness() {
        assert_eq!(SERVICES.len(), 6);
        assert_eq!(SERVICE_NAMES.len(), 6);

        // Every service name should have a corresponding ServiceInfo
        for name in SERVICE_NAMES {
            let info = get_service_info(name);
            assert!(info.is_some(), "Missing ServiceInfo for '{}'", name);
            let info = info.unwrap();
            assert!(!info.binary.is_empty());
            assert!(info.port > 0);
        }
    }

    #[test]
    fn test_service_ports_unique() {
        let mut ports: Vec<u16> = SERVICES.iter().map(|s| s.port).collect();
        ports.sort();
        ports.dedup();
        assert_eq!(ports.len(), SERVICES.len(), "Duplicate ports detected");
    }

    #[test]
    fn test_service_names_unique() {
        let mut names: Vec<&str> = SERVICES.iter().map(|s| s.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SERVICES.len(), "Duplicate names detected");
    }

    #[test]
    fn test_get_service_info_found() {
        let info = get_service_info("notify").unwrap();
        assert_eq!(info.binary, "agentd-notify");
        assert_eq!(info.port, 7004);
    }

    #[test]
    fn test_get_service_info_not_found() {
        assert!(get_service_info("nonexistent").is_none());
    }

    #[test]
    fn test_ask_service_has_extra_env() {
        let info = get_service_info("ask").unwrap();
        assert_eq!(info.extra_env.len(), 1);
        assert_eq!(info.extra_env[0].0, "NOTIFY_SERVICE_URL");
    }

    #[test]
    fn test_detect_platform() {
        // Should not panic on any platform
        let _platform = detect_platform();
    }
}
