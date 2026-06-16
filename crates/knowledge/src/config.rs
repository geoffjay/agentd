//! Configuration for the agentd-knowledge service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid.
//!
//! # Environment Variables
//!
//! | Variable                     | Default                        | Description            |
//! |------------------------------|--------------------------------|------------------------|
//! | `AGENTD_HOST`                | `127.0.0.1`                    | HTTP bind host         |
//! | `AGENTD_KNOWLEDGE_PORT`      | `17011`                        | HTTP listen port       |
//! | `AGENTD_KNOWLEDGE_ROOT`      | platform data dir              | Document storage root  |

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use std::env;

/// Configuration for the agentd-knowledge service.
#[derive(Debug, Clone)]
pub struct KnowledgeConfig {
    /// Bind host (default: `127.0.0.1`)
    pub host: String,
    /// HTTP listen port (default: `17011`)
    pub port: u16,
    /// Root directory for document storage (default: platform data dir)
    pub root: String,
}

impl KnowledgeConfig {
    /// Load configuration from the shared config file and environment variables.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });

        let host = env::var("AGENTD_HOST").unwrap_or(shared.general.host);
        let port = env::var("AGENTD_KNOWLEDGE_PORT")
            .or_else(|_| env::var("AGENTD_PORT"))
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(shared.services.knowledge.port);
        let root = env::var("AGENTD_KNOWLEDGE_ROOT").unwrap_or(shared.services.knowledge.root);

        Self { host, port, root }
    }
}

impl ValidateConfig for KnowledgeConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("knowledge.port must be non-zero");
        }
        if self.root.is_empty() {
            bail!("knowledge.root must not be empty");
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
    fn test_validate_default_passes() {
        let config = KnowledgeConfig {
            host: "127.0.0.1".to_string(),
            port: 17011,
            root: "/tmp/kb".to_string(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config =
            KnowledgeConfig { host: "127.0.0.1".to_string(), port: 0, root: "/tmp/kb".to_string() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_empty_root_fails() {
        let config =
            KnowledgeConfig { host: "127.0.0.1".to_string(), port: 17011, root: String::new() };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_env_override_port() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_KNOWLEDGE_PORT", "7011");
        env::set_var("AGENTD_CONFIG", "/tmp/agentd-nonexistent-kb-test.toml");
        let config = KnowledgeConfig::load();
        env::remove_var("AGENTD_KNOWLEDGE_PORT");
        env::remove_var("AGENTD_CONFIG");
        assert_eq!(config.port, 7011);
    }
}
