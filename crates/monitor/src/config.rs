//! Configuration for the monitor service.
//!
//! All settings can be overridden via environment variables at startup.

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use std::env;

/// Configuration for the monitor service.
///
/// Values are read from environment variables at construction time and fall
/// back to sensible defaults suitable for local development.
///
/// # Examples
///
/// ```
/// use monitor::config::MonitorConfig;
///
/// let config = MonitorConfig::from_env();
/// assert!(config.collection_interval_secs > 0);
/// ```
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// TCP port for the HTTP server (default: 17003 dev)
    pub port: u16,
    /// Seconds between automatic metric collections (default: 30)
    pub collection_interval_secs: u64,
    /// CPU usage % above which an alert is raised (default: 90.0)
    pub cpu_alert_threshold: f32,
    /// Memory usage % above which an alert is raised (default: 90.0)
    pub memory_alert_threshold: f32,
    /// Disk usage % above which an alert is raised (default: 90.0)
    pub disk_alert_threshold: f32,
    /// Maximum number of metric snapshots to retain in memory (default: 120)
    pub history_size: usize,
    /// Base URL of the Prometheus server backing the named-query API
    /// (default: http://127.0.0.1:9090, matching infra/prometheus/).
    pub prometheus_url: String,
}

impl MonitorConfig {
    /// Construct configuration from the shared config file and environment variables.
    ///
    /// Loads base values from [`agentd_common::config::load`], then overlays
    /// legacy service-specific environment variables for backward compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });
        let base = shared.services.monitor;

        Self {
            port: env::var("AGENTD_MONITOR_PORT")
                .or_else(|_| env::var("AGENTD_PORT"))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.port),
            collection_interval_secs: env::var("AGENTD_COLLECTION_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.collection_interval_secs),
            // TODO(#1201): migrate when shared schema adds cpu_alert_threshold
            cpu_alert_threshold: env::var("AGENTD_CPU_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            // TODO(#1201): migrate when shared schema adds memory_alert_threshold
            memory_alert_threshold: env::var("AGENTD_MEMORY_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            // TODO(#1201): migrate when shared schema adds disk_alert_threshold
            disk_alert_threshold: env::var("AGENTD_DISK_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            // TODO(#1201): migrate when shared schema adds history_size
            history_size: env::var("AGENTD_HISTORY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
            // TODO(#1201): migrate when shared schema adds prometheus_url
            prometheus_url: env::var("AGENTD_PROMETHEUS_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string()),
        }
    }

    /// Construct configuration from environment variables with defaults.
    #[deprecated(note = "Use load() instead")]
    pub fn from_env() -> Self {
        Self::load()
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            port: 17003,
            collection_interval_secs: 30,
            cpu_alert_threshold: 90.0,
            memory_alert_threshold: 90.0,
            disk_alert_threshold: 90.0,
            history_size: 120,
            prometheus_url: "http://127.0.0.1:9090".to_string(),
        }
    }
}

impl ValidateConfig for MonitorConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("monitor.port must be non-zero");
        }
        if self.collection_interval_secs == 0 {
            bail!("monitor.collection_interval_secs must be greater than 0");
        }
        for (name, threshold) in [
            ("monitor.cpu_alert_threshold", self.cpu_alert_threshold),
            ("monitor.memory_alert_threshold", self.memory_alert_threshold),
            ("monitor.disk_alert_threshold", self.disk_alert_threshold),
        ] {
            if !(0.0..=100.0).contains(&threshold) {
                bail!("{name} must be between 0.0 and 100.0, got: {threshold}");
            }
        }
        if !self.prometheus_url.starts_with("http://")
            && !self.prometheus_url.starts_with("https://")
        {
            bail!(
                "monitor.prometheus_url must start with http:// or https://, got: {}",
                self.prometheus_url
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MonitorConfig::default();
        assert_eq!(config.port, 17003);
        assert_eq!(config.collection_interval_secs, 30);
        assert!((config.cpu_alert_threshold - 90.0).abs() < 0.01);
        assert!((config.memory_alert_threshold - 90.0).abs() < 0.01);
        assert!((config.disk_alert_threshold - 90.0).abs() < 0.01);
        assert_eq!(config.history_size, 120);
    }

    #[test]
    fn test_config_clone() {
        let config = MonitorConfig::default();
        let cloned = config.clone();
        assert_eq!(config.port, cloned.port);
        assert_eq!(config.history_size, cloned.history_size);
    }

    #[test]
    fn test_validate_default_passes() {
        assert!(MonitorConfig::default().validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = MonitorConfig { port: 0, ..MonitorConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_interval_fails() {
        let config = MonitorConfig { collection_interval_secs: 0, ..MonitorConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_cpu_threshold_above_100_fails() {
        let config = MonitorConfig { cpu_alert_threshold: 101.0, ..MonitorConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_memory_threshold_negative_fails() {
        let config = MonitorConfig { memory_alert_threshold: -1.0, ..MonitorConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_boundary_thresholds_pass() {
        let config = MonitorConfig {
            cpu_alert_threshold: 0.0,
            memory_alert_threshold: 100.0,
            disk_alert_threshold: 50.0,
            ..MonitorConfig::default()
        };
        assert!(config.validate().is_ok());
    }
}
