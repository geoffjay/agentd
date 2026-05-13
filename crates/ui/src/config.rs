use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};

/// Configuration for the UI service.
#[derive(Debug, Clone)]
pub struct UiConfig {
    /// Port to listen on.
    pub port: u16,
    /// Directory containing the built UI static files.
    pub ui_dir: String,
    /// URL of the ask service.
    pub ask_service_url: String,
    /// URL of the notify service.
    pub notify_service_url: String,
    /// URL of the orchestrator service.
    pub orchestrator_service_url: String,
    /// URL of the index service.
    pub index_service_url: String,
}

impl UiConfig {
    /// Build configuration from the shared config file and environment variables.
    ///
    /// Loads base values from [`agentd_common::config::load`], then overlays
    /// legacy service-specific environment variables for backward compatibility.
    ///
    /// - `AGENTD_PORT` — port (default: 17009 for dev, override with env)
    /// - `AGENTD_UI_DIR` — path to built UI assets (default: `./ui/dist`)
    /// - `AGENTD_ASK_SERVICE_URL` — ask service URL (default: `http://localhost:7001`)
    /// - `AGENTD_NOTIFY_SERVICE_URL` — notify service URL (default: `http://localhost:7004`)
    /// - `AGENTD_ORCHESTRATOR_SERVICE_URL` — orchestrator service URL (default: `http://localhost:7006`)
    /// - `AGENTD_INDEX_SERVICE_URL` — index service URL (default: `http://localhost:17012`)
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });
        let base = shared.services.ui;

        Self {
            port: std::env::var("AGENTD_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(base.port),
            ui_dir: std::env::var("AGENTD_UI_DIR").unwrap_or(base.ui_dir),
            ask_service_url: std::env::var("AGENTD_ASK_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7001".to_string()),
            notify_service_url: std::env::var("AGENTD_NOTIFY_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7004".to_string()),
            orchestrator_service_url: std::env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7006".to_string()),
            index_service_url: std::env::var("AGENTD_INDEX_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:17012".to_string()),
        }
    }

    /// Build configuration from environment variables.
    #[deprecated(note = "Use load() instead")]
    pub fn from_env() -> Self {
        Self::load()
    }
}

impl ValidateConfig for UiConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("ui.port must be non-zero");
        }
        if self.ui_dir.is_empty() {
            bail!("ui.ui_dir must not be empty");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_default_passes() {
        let config = UiConfig::load();
        // Default port is 17009 and ui_dir is "./ui/dist" — both valid.
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = UiConfig {
            port: 0,
            ui_dir: "./ui/dist".to_string(),
            ask_service_url: "http://localhost:7001".to_string(),
            notify_service_url: "http://localhost:7004".to_string(),
            orchestrator_service_url: "http://localhost:7006".to_string(),
            index_service_url: "http://localhost:17012".to_string(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_empty_ui_dir_fails() {
        let config = UiConfig {
            port: 17009,
            ui_dir: "".to_string(),
            ask_service_url: "http://localhost:7001".to_string(),
            notify_service_url: "http://localhost:7004".to_string(),
            orchestrator_service_url: "http://localhost:7006".to_string(),
            index_service_url: "http://localhost:17012".to_string(),
        };
        assert!(config.validate().is_err());
    }
}
