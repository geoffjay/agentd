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

/// Policy controlling which tools an agent is allowed to use.
///
/// When a Claude Code agent requests permission to use a tool (via the
/// `can_use_tool` control request), this policy is evaluated to decide
/// whether to allow or deny the request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[derive(Default)]
pub enum ToolPolicy {
    /// Allow all tools without restriction (default).
    #[default]
    AllowAll,
    /// Deny all tool usage.
    DenyAll,
    /// Only allow the listed tools; deny everything else.
    AllowList { tools: Vec<String> },
    /// Allow everything except the listed tools.
    DenyList { tools: Vec<String> },
    /// Hold every tool request for human approval before permitting it.
    RequireApproval,
}

impl ToolPolicy {
    /// Evaluate whether a tool is allowed by this policy.
    ///
    /// Note: `RequireApproval` returns `false` here as a fallback — the actual
    /// approval logic is handled in `websocket.rs` before `evaluate` is called.
    pub fn evaluate(&self, tool_name: &str) -> bool {
        match self {
            ToolPolicy::AllowAll => true,
            ToolPolicy::DenyAll => false,
            ToolPolicy::AllowList { tools } => tools.iter().any(|t| t == tool_name),
            ToolPolicy::DenyList { tools } => !tools.iter().any(|t| t == tool_name),
            ToolPolicy::RequireApproval => false,
        }
    }

    /// Returns the policy mode as a string for logging.
    pub fn mode_str(&self) -> &'static str {
        match self {
            ToolPolicy::AllowAll => "allow_all",
            ToolPolicy::DenyAll => "deny_all",
            ToolPolicy::AllowList { .. } => "allow_list",
            ToolPolicy::DenyList { .. } => "deny_list",
            ToolPolicy::RequireApproval => "require_approval",
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
    /// Tool-use policy for this agent.
    #[serde(default)]
    pub tool_policy: ToolPolicy,
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
    /// Tool-use policy for this agent.
    #[serde(default)]
    pub tool_policy: ToolPolicy,
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

// Re-export pagination types from agentd-common.
#[allow(unused_imports)]
pub use agentd_common::types::{
    clamp_limit, PaginatedResponse, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT,
};

// Re-export shared HealthResponse from agentd-common.
pub use agentd_common::types::HealthResponse;

/// Request body for POST /agents/{id}/message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

/// Response body for POST /agents/{id}/message.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub status: String,
    pub agent_id: Uuid,
}

// -- Tool approval types --

/// Status of a pending tool approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    TimedOut,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Pending => write!(f, "pending"),
            ApprovalStatus::Approved => write!(f, "approved"),
            ApprovalStatus::Denied => write!(f, "denied"),
            ApprovalStatus::TimedOut => write!(f, "timed_out"),
        }
    }
}

impl std::str::FromStr for ApprovalStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ApprovalStatus::Pending),
            "approved" => Ok(ApprovalStatus::Approved),
            "denied" => Ok(ApprovalStatus::Denied),
            "timed_out" => Ok(ApprovalStatus::TimedOut),
            _ => Err(anyhow::anyhow!("Unknown approval status: {}", s)),
        }
    }
}

/// An in-flight tool approval request awaiting human decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub id: Uuid,
    pub agent_id: Uuid,
    /// The WebSocket request_id from the claude control_request message.
    pub request_id: String,
    pub tool_name: String,
    /// Full tool input as JSON (for display in the UI/CLI).
    pub tool_input: serde_json::Value,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    /// When the approval will auto-deny if not acted on.
    pub expires_at: DateTime<Utc>,
}

/// Decision to resolve a pending approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

/// Request body for approval/deny endpoints (allows future extension).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalActionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_policy_allow_all() {
        let policy = ToolPolicy::AllowAll;
        assert!(policy.evaluate("Bash"));
        assert!(policy.evaluate("Read"));
        assert!(policy.evaluate("Write"));
        assert!(policy.evaluate("anything"));
    }

    #[test]
    fn test_tool_policy_deny_all() {
        let policy = ToolPolicy::DenyAll;
        assert!(!policy.evaluate("Bash"));
        assert!(!policy.evaluate("Read"));
        assert!(!policy.evaluate("anything"));
    }

    #[test]
    fn test_tool_policy_allow_list() {
        let policy = ToolPolicy::AllowList { tools: vec!["Read".to_string(), "Grep".to_string()] };
        assert!(policy.evaluate("Read"));
        assert!(policy.evaluate("Grep"));
        assert!(!policy.evaluate("Bash"));
        assert!(!policy.evaluate("Write"));
    }

    #[test]
    fn test_tool_policy_deny_list() {
        let policy = ToolPolicy::DenyList { tools: vec!["Bash".to_string(), "Write".to_string()] };
        assert!(!policy.evaluate("Bash"));
        assert!(!policy.evaluate("Write"));
        assert!(policy.evaluate("Read"));
        assert!(policy.evaluate("Grep"));
    }

    #[test]
    fn test_tool_policy_default_is_allow_all() {
        let policy = ToolPolicy::default();
        assert_eq!(policy, ToolPolicy::AllowAll);
        assert!(policy.evaluate("anything"));
    }

    #[test]
    fn test_tool_policy_serialization_allow_all() {
        let policy = ToolPolicy::AllowAll;
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("allow_all"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ToolPolicy::AllowAll);
    }

    #[test]
    fn test_tool_policy_serialization_deny_list() {
        let policy = ToolPolicy::DenyList { tools: vec!["Bash".to_string(), "Write".to_string()] };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("deny_list"));
        assert!(json.contains("Bash"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_tool_policy_serialization_allow_list() {
        let policy = ToolPolicy::AllowList { tools: vec!["Read".to_string()] };
        let json = serde_json::to_string(&policy).unwrap();

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_tool_policy_empty_allow_list_denies_all() {
        let policy = ToolPolicy::AllowList { tools: vec![] };
        assert!(!policy.evaluate("Read"));
        assert!(!policy.evaluate("Bash"));
    }

    #[test]
    fn test_tool_policy_empty_deny_list_allows_all() {
        let policy = ToolPolicy::DenyList { tools: vec![] };
        assert!(policy.evaluate("Read"));
        assert!(policy.evaluate("Bash"));
    }

    #[test]
    fn test_tool_policy_require_approval() {
        let policy = ToolPolicy::RequireApproval;
        // evaluate returns false as fallback — actual logic is in websocket.rs
        assert!(!policy.evaluate("Bash"));
        assert!(!policy.evaluate("Read"));
    }

    #[test]
    fn test_tool_policy_serialization_require_approval() {
        let policy = ToolPolicy::RequireApproval;
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("require_approval"));

        let deserialized: ToolPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ToolPolicy::RequireApproval);
    }

    #[test]
    fn test_approval_status_display_and_parse() {
        for (status, expected) in [
            (ApprovalStatus::Pending, "pending"),
            (ApprovalStatus::Approved, "approved"),
            (ApprovalStatus::Denied, "denied"),
            (ApprovalStatus::TimedOut, "timed_out"),
        ] {
            assert_eq!(status.to_string(), expected);
            assert_eq!(expected.parse::<ApprovalStatus>().unwrap(), status);
        }
    }

    #[test]
    fn test_tool_policy_mode_str() {
        assert_eq!(ToolPolicy::AllowAll.mode_str(), "allow_all");
        assert_eq!(ToolPolicy::DenyAll.mode_str(), "deny_all");
        assert_eq!(ToolPolicy::RequireApproval.mode_str(), "require_approval");
    }
}
