//! Configuration for the agentd-notify service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid for backward compatibility.
//!
//! # Environment Variables
//!
//! | Variable        | Default       | Description      |
//! |-----------------|---------------|------------------|
//! | `AGENTD_HOST`   | `127.0.0.1`   | HTTP bind host   |
//! | `AGENTD_PORT`   | `17004`       | HTTP listen port |

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use std::env;

/// Configuration for the agentd-notify service.
#[derive(Debug, Clone)]
pub struct NotifyConfig {
    /// Bind host (default: `127.0.0.1`)
    pub host: String,
    /// HTTP listen port (default: `17004`)
    pub port: u16,
}

impl NotifyConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Reads base values from [`agentd_common::config::load`], then overlays
    /// `AGENTD_HOST` and `AGENTD_PORT` environment variables for backward
    /// compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_default();

        let host = env::var("AGENTD_HOST").unwrap_or(shared.general.host);
        let port = env::var("AGENTD_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(shared.services.notify.port);

        Self { host, port }
    }
}

impl ValidateConfig for NotifyConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("notify.port must be non-zero");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_host = env::var("AGENTD_HOST").ok();
        let saved_port = env::var("AGENTD_PORT").ok();
        env::remove_var("AGENTD_HOST");
        env::remove_var("AGENTD_PORT");

        let config = NotifyConfig::load();

        if let Some(v) = saved_host {
            env::set_var("AGENTD_HOST", v);
        }
        if let Some(v) = saved_port {
            env::set_var("AGENTD_PORT", v);
        }

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 17004);
    }

    #[test]
    fn test_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_PORT", "9004");
        let config = NotifyConfig::load();
        env::remove_var("AGENTD_PORT");
        assert_eq!(config.port, 9004);
    }

    #[test]
    fn test_validate_default_passes() {
        let config = NotifyConfig { host: "127.0.0.1".to_string(), port: 17004 };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = NotifyConfig { host: "127.0.0.1".to_string(), port: 0 };
        assert!(config.validate().is_err());
    }
}
