//! Configuration for the agentd-orchestrator service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid for backward compatibility.
//!
//! # Environment Variables
//!
//! | Variable                       | Default                   | Description                          |
//! |--------------------------------|---------------------------|--------------------------------------|
//! | `AGENTD_HOST`                  | `127.0.0.1`               | HTTP bind host                       |
//! | `AGENTD_PORT`                  | `17006`                   | HTTP listen port                     |
//! | `AGENTD_BACKEND`               | `tmux`                    | Execution backend                    |
//! | `AGENTD_DOCKER_IMAGE`          | (docker default)          | Docker image for agent containers    |
//! | `AGENTD_COMMUNICATE_SERVICE_URL`| `http://localhost:17010` | Communicate service URL              |
//! | `AGENTD_RECONCILE_INTERVAL_SECS`| `30`                     | Agent reconciliation interval (secs) |

use std::env;

/// Configuration for the agentd-orchestrator service.
///
/// Note: the execution backend is selected by `wrap::types::BackendType::from_env_strict()`
/// which reads `AGENTD_BACKEND` directly. That value is not duplicated here.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Bind host (default: `127.0.0.1`)
    pub host: String,
    /// HTTP listen port (default: `17006`)
    pub port: u16,
    /// Docker image for agent containers (no default — uses wrap crate default)
    pub docker_image: Option<String>,
    /// Communicate service URL for the message bridge (default: `http://localhost:17010`)
    pub communicate_url: String,
    /// Agent reconciliation interval in seconds (default: `30`)
    pub reconcile_interval_secs: u64,
}

impl OrchestratorConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Reads base values from [`agentd_common::config::load`], then overlays
    /// legacy environment variables for backward compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });
        let base = shared.services.orchestrator;

        let host = env::var("AGENTD_HOST").unwrap_or(shared.general.host);
        let port =
            env::var("AGENTD_PORT").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(base.port);
        let docker_image = env::var("AGENTD_DOCKER_IMAGE").ok();
        let communicate_url =
            env::var("AGENTD_COMMUNICATE_SERVICE_URL").unwrap_or(base.communicate_url);
        let reconcile_interval_secs = env::var("AGENTD_RECONCILE_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(base.reconcile_interval_secs);

        Self { host, port, docker_image, communicate_url, reconcile_interval_secs }
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
        let saved_comm = env::var("AGENTD_COMMUNICATE_SERVICE_URL").ok();
        let saved_reconcile = env::var("AGENTD_RECONCILE_INTERVAL_SECS").ok();
        env::remove_var("AGENTD_HOST");
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_DOCKER_IMAGE");
        env::remove_var("AGENTD_COMMUNICATE_SERVICE_URL");
        env::remove_var("AGENTD_RECONCILE_INTERVAL_SECS");

        let config = OrchestratorConfig::load();

        if let Some(v) = saved_host {
            env::set_var("AGENTD_HOST", v);
        }
        if let Some(v) = saved_port {
            env::set_var("AGENTD_PORT", v);
        }
        if let Some(v) = saved_comm {
            env::set_var("AGENTD_COMMUNICATE_SERVICE_URL", v);
        }
        if let Some(v) = saved_reconcile {
            env::set_var("AGENTD_RECONCILE_INTERVAL_SECS", v);
        }

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 17006);
        assert!(config.docker_image.is_none());
        assert_eq!(config.communicate_url, "http://localhost:17010");
        assert_eq!(config.reconcile_interval_secs, 30);
    }

    #[test]
    fn test_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_PORT", "9006");
        env::set_var("AGENTD_RECONCILE_INTERVAL_SECS", "60");
        let config = OrchestratorConfig::load();
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_RECONCILE_INTERVAL_SECS");
        assert_eq!(config.port, 9006);
        assert_eq!(config.reconcile_interval_secs, 60);
    }
}
