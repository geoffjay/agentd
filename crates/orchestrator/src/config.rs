//! Configuration for the agentd-orchestrator service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid for backward compatibility.
//!
//! # Environment Variables
//!
//! | Variable                       | Default                   | Description                          |
//! |--------------------------------|---------------------------|--------------------------------------|
//! | `AGENTD_HOST`                        | `127.0.0.1`               | HTTP bind host                       |
//! | `AGENTD_ORCHESTRATOR_PORT`           | `17006`                   | HTTP listen port                     |
//! | `AGENTD_PORT`                        | —                         | Fallback port (legacy)               |
//! | `AGENTD_BACKEND`                     | `tmux`                    | Execution backend (overrides config) |
//! | `AGENTD_DOCKER_IMAGE`                | (docker default)          | Docker image for agent containers    |
//! | `AGENTD_COMMUNICATE_SERVICE_URL`     | `http://localhost:17010`  | Communicate service URL              |
//! | `AGENTD_RECONCILE_INTERVAL_SECS`     | `30`                      | Agent reconciliation interval (secs) |
//! | `AGENTD_ORCHESTRATOR_SUBPROCESS_PATH`| `""`                      | PATH injected into spawned subprocesses |

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
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
    /// Execution backend: `"tmux"`, `"docker"`, `"pty"`, or `"subprocess"` (default: `"tmux"`)
    pub backend: String,
    /// Communicate service URL for the message bridge (default: `http://localhost:17010`)
    pub communicate_url: String,
    /// Agent reconciliation interval in seconds (default: `30`)
    pub reconcile_interval_secs: u64,
    /// PATH to inject into spawned subprocesses (empty = inherit orchestrator PATH)
    pub subprocess_path: String,
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
        let port = env::var("AGENTD_ORCHESTRATOR_PORT")
            .or_else(|_| env::var("AGENTD_PORT"))
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(base.port);
        let docker_image = env::var("AGENTD_DOCKER_IMAGE").ok();
        let backend = env::var("AGENTD_BACKEND").unwrap_or(base.backend);
        let communicate_url =
            env::var("AGENTD_COMMUNICATE_SERVICE_URL").unwrap_or(base.communicate_url);
        let reconcile_interval_secs = env::var("AGENTD_RECONCILE_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(base.reconcile_interval_secs);
        let subprocess_path =
            env::var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH").unwrap_or(base.subprocess_path);

        Self {
            host,
            port,
            docker_image,
            backend,
            communicate_url,
            reconcile_interval_secs,
            subprocess_path,
        }
    }
}

impl ValidateConfig for OrchestratorConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("orchestrator.port must be non-zero");
        }
        if !self.communicate_url.starts_with("http://")
            && !self.communicate_url.starts_with("https://")
        {
            bail!(
                "orchestrator.communicate_url must be a valid HTTP/HTTPS URL, got: {}",
                self.communicate_url
            );
        }
        if self.reconcile_interval_secs == 0 {
            bail!("orchestrator.reconcile_interval_secs must be greater than 0");
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
        let saved_comm = env::var("AGENTD_COMMUNICATE_SERVICE_URL").ok();
        let saved_reconcile = env::var("AGENTD_RECONCILE_INTERVAL_SECS").ok();
        let saved_subpath = env::var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH").ok();
        let saved_cfg = env::var("AGENTD_CONFIG").ok();
        env::remove_var("AGENTD_HOST");
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_DOCKER_IMAGE");
        env::remove_var("AGENTD_COMMUNICATE_SERVICE_URL");
        env::remove_var("AGENTD_RECONCILE_INTERVAL_SECS");
        env::remove_var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH");
        // Point away from the real config file so compiled defaults are used.
        env::set_var("AGENTD_CONFIG", "/tmp/agentd-test-nonexistent-defaults.toml");

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
        if let Some(v) = saved_subpath {
            env::set_var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH", v);
        }
        match saved_cfg {
            Some(v) => env::set_var("AGENTD_CONFIG", v),
            None => env::remove_var("AGENTD_CONFIG"),
        }

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 17006);
        assert!(config.docker_image.is_none());
        assert_eq!(config.communicate_url, "http://localhost:17010");
        assert_eq!(config.reconcile_interval_secs, 30);
        assert_eq!(config.subprocess_path, "");
    }

    #[test]
    fn test_subprocess_path_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH", "/usr/local/bin:/usr/bin");
        let config = OrchestratorConfig::load();
        env::remove_var("AGENTD_ORCHESTRATOR_SUBPROCESS_PATH");
        assert_eq!(config.subprocess_path, "/usr/local/bin:/usr/bin");
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

    #[test]
    fn test_service_specific_port_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_ORCHESTRATOR_PORT", "7006");
        let config = OrchestratorConfig::load();
        env::remove_var("AGENTD_ORCHESTRATOR_PORT");
        assert_eq!(config.port, 7006);
    }

    #[test]
    fn test_service_specific_port_takes_priority_over_generic() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_ORCHESTRATOR_PORT", "7006");
        env::set_var("AGENTD_PORT", "9006");
        let config = OrchestratorConfig::load();
        env::remove_var("AGENTD_ORCHESTRATOR_PORT");
        env::remove_var("AGENTD_PORT");
        assert_eq!(config.port, 7006);
    }

    #[test]
    fn test_validate_default_passes() {
        let config = OrchestratorConfig {
            host: "127.0.0.1".to_string(),
            port: 17006,
            docker_image: None,
            backend: "tmux".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            reconcile_interval_secs: 30,
            subprocess_path: String::new(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = OrchestratorConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
            docker_image: None,
            backend: "tmux".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            reconcile_interval_secs: 30,
            subprocess_path: String::new(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_bad_communicate_url_fails() {
        let config = OrchestratorConfig {
            host: "127.0.0.1".to_string(),
            port: 17006,
            docker_image: None,
            backend: "tmux".to_string(),
            communicate_url: "localhost:17010".to_string(),
            reconcile_interval_secs: 30,
            subprocess_path: String::new(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_reconcile_interval_fails() {
        let config = OrchestratorConfig {
            host: "127.0.0.1".to_string(),
            port: 17006,
            docker_image: None,
            backend: "tmux".to_string(),
            communicate_url: "http://localhost:17010".to_string(),
            reconcile_interval_secs: 0,
            subprocess_path: String::new(),
        };
        assert!(config.validate().is_err());
    }
}
