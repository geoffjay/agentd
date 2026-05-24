use agentd_common::config::AgentdConfig;

pub struct TuiConfig {
    pub orchestrator_url: String,
    pub memory_url: String,
    pub prometheus_url: String,
    pub refresh_interval_secs: u64,
    pub agentd_config: AgentdConfig,
}

impl TuiConfig {
    pub fn from_agentd_config(cfg: AgentdConfig) -> Self {
        let host = &cfg.general.host;
        let orchestrator_url = std::env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
            .unwrap_or_else(|_| format!("http://{}:{}", host, cfg.services.orchestrator.port));
        let memory_url = std::env::var("AGENTD_MEMORY_SERVICE_URL")
            .unwrap_or_else(|_| format!("http://{}:{}", host, cfg.services.memory.port));
        let prometheus_url = std::env::var("AGENTD_PROMETHEUS_URL")
            .unwrap_or_else(|_| "http://localhost:9090".to_string());
        Self {
            orchestrator_url,
            memory_url,
            prometheus_url,
            refresh_interval_secs: 5,
            agentd_config: cfg,
        }
    }
}
