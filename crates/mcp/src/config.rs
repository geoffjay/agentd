//! Configuration for the agentd-mcp server.
//!
//! All configuration is read from environment variables at startup.
//! Each agentd service has a corresponding URL that defaults to the
//! standard localhost port for that service.

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use std::env;

/// Configuration for connecting to agentd services.
#[derive(Debug, Clone)]
pub struct AgentdMcpConfig {
    /// Orchestrator service URL (default: `http://127.0.0.1:17006`)
    pub orchestrator_url: String,
    /// Communicate service URL (default: `http://127.0.0.1:17010`)
    pub communicate_url: String,
    /// Memory service URL (default: `http://127.0.0.1:17008`)
    pub memory_url: String,
    /// Notify service URL (default: `http://127.0.0.1:17004`)
    pub notify_url: String,
    /// Ask service URL (default: `http://127.0.0.1:17001`)
    pub ask_url: String,
    /// Wrap service URL (default: `http://127.0.0.1:17005`)
    pub wrap_url: String,
    /// Monitor service URL (default: `http://127.0.0.1:17003`)
    pub monitor_url: String,
    /// Hook service URL (default: `http://127.0.0.1:17002`)
    pub hook_url: String,
}

impl AgentdMcpConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Loads base values from [`agentd_common::config::load`], then overlays
    /// legacy service-specific environment variables for backward compatibility.
    ///
    /// # Environment Variables
    ///
    /// | Variable                        | Default                     |
    /// |---------------------------------|-----------------------------|
    /// | `AGENTD_ORCHESTRATOR_URL`       | `http://127.0.0.1:17006`   |
    /// | `AGENTD_COMMUNICATE_URL`        | `http://127.0.0.1:17010`   |
    /// | `AGENTD_MEMORY_URL`             | `http://127.0.0.1:17008`   |
    /// | `AGENTD_NOTIFY_URL`             | `http://127.0.0.1:17004`   |
    /// | `AGENTD_ASK_URL`                | `http://127.0.0.1:17001`   |
    /// | `AGENTD_WRAP_URL`               | `http://127.0.0.1:17005`   |
    /// | `AGENTD_MONITOR_URL`            | `http://127.0.0.1:17003`   |
    /// | `AGENTD_HOOK_URL`               | `http://127.0.0.1:17002`   |
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_default();
        let base = shared.services.mcp;

        Self {
            orchestrator_url: env::var("AGENTD_ORCHESTRATOR_URL").unwrap_or(base.orchestrator_url),
            communicate_url: env::var("AGENTD_COMMUNICATE_URL").unwrap_or(base.communicate_url),
            memory_url: env::var("AGENTD_MEMORY_URL").unwrap_or(base.memory_url),
            notify_url: env::var("AGENTD_NOTIFY_URL").unwrap_or(base.notify_url),
            ask_url: env::var("AGENTD_ASK_URL").unwrap_or(base.ask_url),
            wrap_url: env::var("AGENTD_WRAP_URL").unwrap_or(base.wrap_url),
            monitor_url: env::var("AGENTD_MONITOR_URL").unwrap_or(base.monitor_url),
            hook_url: env::var("AGENTD_HOOK_URL").unwrap_or(base.hook_url),
        }
    }

    /// Load configuration from environment variables.
    #[deprecated(note = "Use load() instead")]
    pub fn from_env() -> Self {
        Self::load()
    }
}

impl ValidateConfig for AgentdMcpConfig {
    fn validate(&self) -> Result<()> {
        let urls = [
            ("mcp.orchestrator_url", self.orchestrator_url.as_str()),
            ("mcp.communicate_url", self.communicate_url.as_str()),
            ("mcp.memory_url", self.memory_url.as_str()),
            ("mcp.notify_url", self.notify_url.as_str()),
            ("mcp.ask_url", self.ask_url.as_str()),
            ("mcp.wrap_url", self.wrap_url.as_str()),
            ("mcp.monitor_url", self.monitor_url.as_str()),
            ("mcp.hook_url", self.hook_url.as_str()),
        ];
        for (field, url) in urls {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                bail!("{field} must be a valid HTTP/HTTPS URL, got: {url}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[allow(deprecated)]
    #[test]
    fn test_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vars = [
            "AGENTD_ORCHESTRATOR_URL",
            "AGENTD_COMMUNICATE_URL",
            "AGENTD_MEMORY_URL",
            "AGENTD_NOTIFY_URL",
            "AGENTD_ASK_URL",
            "AGENTD_WRAP_URL",
            "AGENTD_MONITOR_URL",
            "AGENTD_HOOK_URL",
        ];
        let saved: Vec<_> = vars.iter().map(|k| (k, env::var(k).ok())).collect();
        for k in &vars {
            env::remove_var(k);
        }

        let config = AgentdMcpConfig::from_env();
        assert_eq!(config.orchestrator_url, "http://127.0.0.1:17006");
        assert_eq!(config.communicate_url, "http://127.0.0.1:17010");
        assert_eq!(config.memory_url, "http://127.0.0.1:17008");
        assert_eq!(config.notify_url, "http://127.0.0.1:17004");
        assert_eq!(config.ask_url, "http://127.0.0.1:17001");
        assert_eq!(config.wrap_url, "http://127.0.0.1:17005");
        assert_eq!(config.monitor_url, "http://127.0.0.1:17003");
        assert_eq!(config.hook_url, "http://127.0.0.1:17002");

        for (k, v) in saved {
            if let Some(val) = v {
                env::set_var(k, val);
            }
        }
    }

    #[allow(deprecated)]
    #[test]
    fn test_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_ORCHESTRATOR_URL", "http://10.0.0.1:9000");
        let config = AgentdMcpConfig::from_env();
        env::remove_var("AGENTD_ORCHESTRATOR_URL");
        assert_eq!(config.orchestrator_url, "http://10.0.0.1:9000");
    }

    #[test]
    fn test_validate_default_passes() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vars = [
            "AGENTD_ORCHESTRATOR_URL",
            "AGENTD_COMMUNICATE_URL",
            "AGENTD_MEMORY_URL",
            "AGENTD_NOTIFY_URL",
            "AGENTD_ASK_URL",
            "AGENTD_WRAP_URL",
            "AGENTD_MONITOR_URL",
            "AGENTD_HOOK_URL",
        ];
        let saved: Vec<_> = vars.iter().map(|k| (k, env::var(k).ok())).collect();
        for k in &vars {
            env::remove_var(k);
        }
        let config = AgentdMcpConfig::load();
        let result = config.validate();
        for (k, v) in saved {
            if let Some(val) = v {
                env::set_var(k, val);
            }
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_bad_url_fails() {
        let config = AgentdMcpConfig {
            orchestrator_url: "not-a-url".to_string(),
            communicate_url: "http://127.0.0.1:17010".to_string(),
            memory_url: "http://127.0.0.1:17008".to_string(),
            notify_url: "http://127.0.0.1:17004".to_string(),
            ask_url: "http://127.0.0.1:17001".to_string(),
            wrap_url: "http://127.0.0.1:17005".to_string(),
            monitor_url: "http://127.0.0.1:17003".to_string(),
            hook_url: "http://127.0.0.1:17002".to_string(),
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("mcp.orchestrator_url"));
    }
}
