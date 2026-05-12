//! Configuration for the agentd-ask service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid for backward compatibility.
//!
//! # Environment Variables
//!
//! | Variable                  | Default                    | Description               |
//! |---------------------------|----------------------------|---------------------------|
//! | `AGENTD_HOST`             | `0.0.0.0`                  | HTTP bind host            |
//! | `AGENTD_PORT`             | `17001`                    | HTTP listen port          |
//! | `AGENTD_ORCHESTRATOR_URL` | `http://localhost:17006`   | Orchestrator callback URL |

use std::env;

/// Configuration for the agentd-ask service.
#[derive(Debug, Clone)]
pub struct AskConfig {
    /// Bind host (default: `0.0.0.0`)
    pub host: String,
    /// HTTP listen port (default: `17001`)
    pub port: u16,
    /// Orchestrator callback URL (default: `http://localhost:17006`)
    pub orchestrator_url: String,
}

impl AskConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Reads base values from [`agentd_common::config::load`], then overlays
    /// legacy environment variables for backward compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_default();
        let base = shared.services.ask;

        // ask historically defaulted to 0.0.0.0 (accept from any interface)
        let host = env::var("AGENTD_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port =
            env::var("AGENTD_PORT").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(base.port);
        let orchestrator_url = env::var("AGENTD_ORCHESTRATOR_URL").unwrap_or(base.orchestrator_url);

        Self { host, port, orchestrator_url }
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
        let saved_orch = env::var("AGENTD_ORCHESTRATOR_URL").ok();
        env::remove_var("AGENTD_HOST");
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_ORCHESTRATOR_URL");

        let config = AskConfig::load();

        if let Some(v) = saved_host {
            env::set_var("AGENTD_HOST", v);
        }
        if let Some(v) = saved_port {
            env::set_var("AGENTD_PORT", v);
        }
        if let Some(v) = saved_orch {
            env::set_var("AGENTD_ORCHESTRATOR_URL", v);
        }

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 17001);
        assert_eq!(config.orchestrator_url, "http://localhost:17006");
    }

    #[test]
    fn test_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_PORT", "9001");
        env::set_var("AGENTD_ORCHESTRATOR_URL", "http://10.0.0.1:17006");
        let config = AskConfig::load();
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_ORCHESTRATOR_URL");
        assert_eq!(config.port, 9001);
        assert_eq!(config.orchestrator_url, "http://10.0.0.1:17006");
    }
}
