//! Configuration for the agentd-mcp server.
//!
//! All fields are scaffold stubs used by tools added in issues #250–#256.
#![allow(dead_code)]
//!
//! All configuration is read from environment variables at startup.
//! Each agentd service has a corresponding URL that defaults to the
//! standard localhost port for that service.

use std::env;

/// Configuration for connecting to agentd services.
#[derive(Debug, Clone)]
pub struct AgentdMcpConfig {
    /// Orchestrator service URL (default: `http://127.0.0.1:17000`)
    pub orchestrator_url: String,
    /// Communicate service URL (default: `http://127.0.0.1:17010`)
    pub communicate_url: String,
    /// Memory service URL (default: `http://127.0.0.1:17008`)
    pub memory_url: String,
    /// Notify service URL (default: `http://127.0.0.1:17001`)
    pub notify_url: String,
    /// Ask service URL (default: `http://127.0.0.1:17002`)
    pub ask_url: String,
    /// Wrap service URL (default: `http://127.0.0.1:17003`)
    pub wrap_url: String,
}

impl AgentdMcpConfig {
    /// Load configuration from environment variables.
    ///
    /// # Environment Variables
    ///
    /// | Variable                        | Default                     |
    /// |---------------------------------|-----------------------------|
    /// | `AGENTD_ORCHESTRATOR_URL`       | `http://127.0.0.1:17000`   |
    /// | `AGENTD_COMMUNICATE_URL`        | `http://127.0.0.1:17010`   |
    /// | `AGENTD_MEMORY_URL`             | `http://127.0.0.1:17008`   |
    /// | `AGENTD_NOTIFY_URL`             | `http://127.0.0.1:17001`   |
    /// | `AGENTD_ASK_URL`                | `http://127.0.0.1:17002`   |
    /// | `AGENTD_WRAP_URL`               | `http://127.0.0.1:17003`   |
    pub fn from_env() -> Self {
        Self {
            orchestrator_url: env::var("AGENTD_ORCHESTRATOR_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17000".to_string()),
            communicate_url: env::var("AGENTD_COMMUNICATE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17010".to_string()),
            memory_url: env::var("AGENTD_MEMORY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17008".to_string()),
            notify_url: env::var("AGENTD_NOTIFY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17001".to_string()),
            ask_url: env::var("AGENTD_ASK_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17002".to_string()),
            wrap_url: env::var("AGENTD_WRAP_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:17003".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        // Temporarily clear env vars so defaults apply
        let vars = [
            "AGENTD_ORCHESTRATOR_URL",
            "AGENTD_COMMUNICATE_URL",
            "AGENTD_MEMORY_URL",
            "AGENTD_NOTIFY_URL",
            "AGENTD_ASK_URL",
            "AGENTD_WRAP_URL",
        ];
        let saved: Vec<_> = vars.iter().map(|k| (k, env::var(k).ok())).collect();
        for k in &vars {
            env::remove_var(k);
        }

        let config = AgentdMcpConfig::from_env();
        assert_eq!(config.orchestrator_url, "http://127.0.0.1:17000");
        assert_eq!(config.communicate_url, "http://127.0.0.1:17010");
        assert_eq!(config.memory_url, "http://127.0.0.1:17008");
        assert_eq!(config.notify_url, "http://127.0.0.1:17001");
        assert_eq!(config.ask_url, "http://127.0.0.1:17002");
        assert_eq!(config.wrap_url, "http://127.0.0.1:17003");

        // Restore
        for (k, v) in saved {
            if let Some(val) = v {
                env::set_var(k, val);
            }
        }
    }

    #[test]
    fn test_env_override() {
        env::set_var("AGENTD_ORCHESTRATOR_URL", "http://10.0.0.1:9000");
        let config = AgentdMcpConfig::from_env();
        assert_eq!(config.orchestrator_url, "http://10.0.0.1:9000");
        env::remove_var("AGENTD_ORCHESTRATOR_URL");
    }
}
