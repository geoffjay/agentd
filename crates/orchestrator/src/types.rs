use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an agent managed by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent record created, not yet running.
    Pending,
    /// Agent is running in a tmux session.
    Running,
    /// Agent was explicitly stopped.
    Stopped,
    /// Agent process failed or crashed.
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Pending => write!(f, "pending"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for AgentStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(AgentStatus::Pending),
            "running" => Ok(AgentStatus::Running),
            "stopped" => Ok(AgentStatus::Stopped),
            "failed" => Ok(AgentStatus::Failed),
            _ => Err(anyhow::anyhow!("Unknown agent status: {}", s)),
        }
    }
}

/// Configuration for spawning an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Working directory for the agent process.
    pub working_dir: String,
    /// OS user to run the agent as (optional, defaults to current user).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Shell to run the agent in (e.g., "bash", "zsh").
    #[serde(default = "default_shell")]
    pub shell: String,
    /// If true, start claude in normal interactive mode without WebSocket.
    #[serde(default)]
    pub interactive: bool,
    /// Initial prompt to execute the claude session with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// If true, start the session with --worktree.
    #[serde(default)]
    pub worktree: bool,
    /// System prompt to use for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

fn default_shell() -> String {
    "zsh".to_string()
}

/// A managed AI agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub config: AgentConfig,
    /// Name of the tmux session hosting this agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Agent {
    pub fn new(name: String, config: AgentConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            status: AgentStatus::Pending,
            config,
            tmux_session: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Request body for POST /agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub working_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default = "default_shell")]
    pub shell: String,
    /// If true, start claude in normal interactive mode without WebSocket.
    #[serde(default)]
    pub interactive: bool,
    /// Initial prompt to execute the claude session with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// If true, start the session with --worktree.
    #[serde(default)]
    pub worktree: bool,
    /// System prompt to use for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

/// Response body for agent endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub config: AgentConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            status: agent.status,
            config: agent.config,
            tmux_session: agent.tmux_session,
            created_at: agent.created_at,
            updated_at: agent.updated_at,
        }
    }
}

/// Health check response.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub agents_active: usize,
}
