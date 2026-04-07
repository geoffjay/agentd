/// Configuration for the UI service.
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
    /// Build configuration from environment variables.
    ///
    /// - `AGENTD_PORT` — port (default: 17009 for dev, override with env)
    /// - `AGENTD_UI_DIR` — path to built UI assets (default: `./ui/dist`)
    /// - `AGENTD_ASK_SERVICE_URL` — ask service URL (default: `http://localhost:7001`)
    /// - `AGENTD_NOTIFY_SERVICE_URL` — notify service URL (default: `http://localhost:7004`)
    /// - `AGENTD_ORCHESTRATOR_SERVICE_URL` — orchestrator service URL (default: `http://localhost:7006`)
    /// - `AGENTD_INDEX_SERVICE_URL` — index service URL (default: `http://localhost:17012`)
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("AGENTD_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(17009),
            ui_dir: std::env::var("AGENTD_UI_DIR").unwrap_or_else(|_| "./ui/dist".to_string()),
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
}
