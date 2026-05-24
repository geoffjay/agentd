use agentd_common::config::AgentdConfig;

pub struct TuiConfig {
    pub orchestrator_url: String,
    pub refresh_interval_secs: u64,
    pub memory_url: String,
    pub agentd_config: AgentdConfig,
}

impl TuiConfig {
    pub fn from_agentd_config(cfg: AgentdConfig) -> Self {
        let host = &cfg.general.host;
        let orchestrator_url = format!("http://{}:{}", host, cfg.services.orchestrator.port);
        let memory_url = format!("http://{}:{}", host, cfg.services.memory.port);
        Self {
            orchestrator_url,
            refresh_interval_secs: 5,
            memory_url,
            agentd_config: cfg,
        }
    }
}
