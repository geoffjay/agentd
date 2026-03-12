//! Configuration for the monitor service.
//!
//! All settings can be overridden via environment variables at startup.

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
}

impl MonitorConfig {
    /// Construct configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        Self {
            port: env::var("AGENTD_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(17003),
            collection_interval_secs: env::var("AGENTD_COLLECTION_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            cpu_alert_threshold: env::var("AGENTD_CPU_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            memory_alert_threshold: env::var("AGENTD_MEMORY_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            disk_alert_threshold: env::var("AGENTD_DISK_ALERT_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90.0),
            history_size: env::var("AGENTD_HISTORY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
        }
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
        }
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
}
