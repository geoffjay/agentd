pub struct TuiConfig {
    pub orchestrator_url: String,
    pub refresh_interval_secs: u64,
}

impl TuiConfig {
    pub fn from_env() -> Self {
        Self {
            orchestrator_url: std::env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7006".to_string()),
            refresh_interval_secs: 5,
        }
    }
}
