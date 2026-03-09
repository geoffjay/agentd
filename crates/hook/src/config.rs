//! Configuration for the hook service.
//!
//! All settings can be overridden via environment variables at startup.

use std::env;

/// Configuration for the hook daemon.
///
/// Values are read from environment variables with sensible defaults.
///
/// # Examples
///
/// ```
/// use hook::config::HookConfig;
///
/// let config = HookConfig::from_env();
/// assert_eq!(config.port, 17002);
/// ```
#[derive(Debug, Clone)]
pub struct HookConfig {
    /// TCP port for the HTTP server (default: 17002 dev)
    pub port: u16,

    /// Maximum number of events to retain in memory (default: 500)
    pub history_size: usize,

    /// Send a notification when a command exits with a non-zero code (default: true)
    pub notify_on_failure: bool,

    /// Send a notification when a command runs longer than the threshold (default: true)
    pub notify_on_long_running: bool,

    /// Minimum duration in milliseconds to consider a command "long-running" (default: 30_000)
    pub long_running_threshold_ms: u64,

    /// Optional URL for the BAML analysis server.
    ///
    /// When set, shell events will be forwarded to BAML for AI-powered analysis.
    /// Leave unset (or empty) to skip BAML analysis.
    pub baml_url: Option<String>,

    /// Optional URL for the notification service.
    ///
    /// When set, notable events will be forwarded as notifications.
    pub notify_service_url: Option<String>,
}

impl HookConfig {
    /// Construct configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let baml_url = env::var("BAML_URL").ok().filter(|s| !s.is_empty());
        let notify_service_url = env::var("NOTIFY_SERVICE_URL").ok().filter(|s| !s.is_empty());

        Self {
            port: env::var("PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(17002),
            history_size: env::var("HISTORY_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(500),
            notify_on_failure: env::var("NOTIFY_ON_FAILURE")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            notify_on_long_running: env::var("NOTIFY_ON_LONG_RUNNING")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            long_running_threshold_ms: env::var("LONG_RUNNING_THRESHOLD_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30_000),
            baml_url,
            notify_service_url,
        }
    }
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            port: 17002,
            history_size: 500,
            notify_on_failure: true,
            notify_on_long_running: true,
            long_running_threshold_ms: 30_000,
            baml_url: None,
            notify_service_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HookConfig::default();
        assert_eq!(config.port, 17002);
        assert_eq!(config.history_size, 500);
        assert!(config.notify_on_failure);
        assert!(config.notify_on_long_running);
        assert_eq!(config.long_running_threshold_ms, 30_000);
        assert!(config.baml_url.is_none());
        assert!(config.notify_service_url.is_none());
    }

    #[test]
    fn test_config_clone() {
        let config = HookConfig::default();
        let cloned = config.clone();
        assert_eq!(config.port, cloned.port);
        assert_eq!(config.history_size, cloned.history_size);
    }
}
