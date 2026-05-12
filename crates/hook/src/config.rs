//! Configuration for the hook service.
//!
//! All settings can be overridden via environment variables at startup.

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
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

    /// Optional URL for the notification service.
    ///
    /// When set, notable events will be forwarded as notifications.
    pub notify_service_url: Option<String>,
}

impl HookConfig {
    /// Construct configuration from the shared config file and environment variables.
    ///
    /// Loads base values from [`agentd_common::config::load`], then overlays
    /// legacy service-specific environment variables for backward compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });
        let base = shared.services.hook;

        let notify_service_url = env::var("AGENTD_NOTIFY_SERVICE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or(base.notify_service_url);

        Self {
            port: env::var("AGENTD_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(base.port),
            history_size: env::var("AGENTD_HISTORY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.history_size),
            // TODO(#1201): migrate when shared schema adds notify_on_failure
            notify_on_failure: env::var("AGENTD_NOTIFY_ON_FAILURE")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            // TODO(#1201): migrate when shared schema adds notify_on_long_running
            notify_on_long_running: env::var("AGENTD_NOTIFY_ON_LONG_RUNNING")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            // TODO(#1201): migrate when shared schema adds long_running_threshold_ms
            long_running_threshold_ms: env::var("AGENTD_LONG_RUNNING_THRESHOLD_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30_000),
            notify_service_url,
        }
    }

    /// Construct configuration from environment variables with defaults.
    #[deprecated(note = "Use load() instead")]
    pub fn from_env() -> Self {
        Self::load()
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
            notify_service_url: None,
        }
    }
}

impl ValidateConfig for HookConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("hook.port must be non-zero");
        }
        if self.history_size == 0 {
            bail!("hook.history_size must be greater than 0");
        }
        if let Some(ref url) = self.notify_service_url {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                bail!("hook.notify_service_url must be a valid HTTP/HTTPS URL, got: {url}");
            }
        }
        Ok(())
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
        assert!(config.notify_service_url.is_none());
    }

    #[test]
    fn test_config_clone() {
        let config = HookConfig::default();
        let cloned = config.clone();
        assert_eq!(config.port, cloned.port);
        assert_eq!(config.history_size, cloned.history_size);
    }

    #[test]
    fn test_validate_default_passes() {
        assert!(HookConfig::default().validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = HookConfig { port: 0, ..HookConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_history_size_fails() {
        let config = HookConfig { history_size: 0, ..HookConfig::default() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_notify_url_fails() {
        let config = HookConfig {
            notify_service_url: Some("not-a-url".to_string()),
            ..HookConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_valid_notify_url_passes() {
        let config = HookConfig {
            notify_service_url: Some("http://notify:17004".to_string()),
            ..HookConfig::default()
        };
        assert!(config.validate().is_ok());
    }
}
