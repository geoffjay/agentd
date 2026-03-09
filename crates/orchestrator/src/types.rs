use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Model to use for the claude session.
    /// Maps to --model flag. Accepts aliases (sonnet, opus, haiku)
    /// or full model names (claude-sonnet-4-6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Environment variables to set when launching the agent.
    /// Commonly used for ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// If set, automatically clear the agent's context when the cumulative
    /// input-token count for the current session exceeds this threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_clear_threshold: Option<u64>,
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
    /// Model to use for the claude session.
    /// Maps to --model flag. Accepts aliases (sonnet, opus, haiku)
    /// or full model names (claude-sonnet-4-6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Environment variables to set when launching the agent.
    /// Commonly used for ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    /// If set, automatically clear the agent's context when the cumulative
    /// input-token count for the current session exceeds this threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_clear_threshold: Option<u64>,
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
        // Redact env values — keys are shown, but values are replaced with "***"
        // to avoid leaking secrets (API keys, tokens) via the REST API.
        let mut config = agent.config;
        config.env = config.env.into_keys().map(|k| (k, "***".to_string())).collect();
        Self {
            id: agent.id,
            name: agent.name,
            status: agent.status,
            config,
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

/// Request body for PUT /agents/{id}/model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModelRequest {
    /// Model to use (e.g. "sonnet", "opus", "haiku", "claude-sonnet-4-6").
    /// Use `null` to clear the model and inherit Claude Code's default.
    pub model: Option<String>,
    /// If true, restart the agent process immediately with the new model.
    /// If false (default), the model change takes effect on next restart.
    #[serde(default)]
    pub restart: bool,
}

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

// -- Usage tracking and context management types --

/// Token counts, cost, and timing from a single `result` message emitted by
/// the Claude Code SDK.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub total_cost_usd: f64,
    pub num_turns: u64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
}

/// Session-level aggregated usage — shared shape for both the active session
/// and the cumulative lifetime totals in [`AgentUsageStats`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub total_cost_usd: f64,
    pub num_turns: u64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    /// Number of `result` messages counted in this session.
    pub result_count: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// Per-agent aggregated usage statistics, including the active session and
/// lifetime cumulative totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsageStats {
    pub agent_id: Uuid,
    /// Stats for the currently-active session, if one is in progress.
    pub current_session: Option<SessionUsage>,
    /// Aggregate totals across all completed and current sessions.
    pub cumulative: SessionUsage,
    /// Total number of sessions (including the current one, if any).
    pub session_count: u32,
}

/// Structured information passed to a [`ResultCallback`] when an agent
/// completes a task.  Replaces the previous `(Uuid, bool)` tuple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultInfo {
    pub agent_id: Uuid,
    pub is_error: bool,
    /// Token/cost/timing snapshot parsed from the `result` message, if present.
    pub usage: Option<UsageSnapshot>,
}

/// Request body for POST /agents/{id}/clear-context.
///
/// Currently has no required fields; reserved for future options (e.g. forcing
/// a checkpoint even when under threshold).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClearContextRequest {}

/// Response body for POST /agents/{id}/clear-context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearContextResponse {
    pub agent_id: Uuid,
    /// Usage statistics at the moment the context was cleared.
    pub session_usage: Option<SessionUsage>,
    /// The session number that will be used going forward (1-based).
    pub new_session_number: u32,
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
    fn test_agent_config_model_serialization() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: Some("opus".to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"model\":\"opus\""));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("opus".to_string()));
    }

    #[test]
    fn test_agent_config_model_none_omitted() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: HashMap::new(),
            auto_clear_threshold: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("model"));
    }

    #[test]
    fn test_create_agent_request_model_field() {
        let request = CreateAgentRequest {
            name: "test".to_string(),
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: Some("sonnet".to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"sonnet\""));

        let deserialized: CreateAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_agent_config_env_serialization() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-test-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());

        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: env.clone(),
            auto_clear_threshold: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("ANTHROPIC_API_KEY"));
        assert!(json.contains("sk-test-key"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.env, env);
    }

    #[test]
    fn test_agent_config_env_empty_omitted() {
        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: HashMap::new(),
            auto_clear_threshold: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"env\""));

        // Deserializing without env field gives empty map
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.env.is_empty());
    }

    #[test]
    fn test_agent_config_env_default_from_missing_field() {
        // Backward compatibility: old JSON without env field should deserialize to empty map
        let json = r#"{"working_dir":"/tmp","shell":"zsh","tool_policy":{"mode":"allow_all"}}"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert!(config.env.is_empty());
    }

    #[test]
    fn test_create_agent_request_env_field() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-test".to_string());

        let request = CreateAgentRequest {
            name: "test".to_string(),
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: env.clone(),
            auto_clear_threshold: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("ANTHROPIC_API_KEY"));

        let deserialized: CreateAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.env, env);
    }

    #[test]
    fn test_agent_response_env_values_redacted() {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "sk-secret-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "https://example.com".to_string());

        let config = AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            tool_policy: ToolPolicy::default(),
            model: None,
            env,
            auto_clear_threshold: None,
        };
        let agent = Agent::new("test".to_string(), config);
        let response = AgentResponse::from(agent);

        // Keys should be present, but values should be redacted
        assert_eq!(response.config.env.get("ANTHROPIC_API_KEY"), Some(&"***".to_string()));
        assert_eq!(response.config.env.get("ANTHROPIC_BASE_URL"), Some(&"***".to_string()));
        // Secret value must not appear
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("sk-secret-key"));
    }

    #[test]
    fn test_set_model_request_serialization() {
        let request = SetModelRequest { model: Some("opus".to_string()), restart: true };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"opus\""));
        assert!(json.contains("\"restart\":true"));

        let deserialized: SetModelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, Some("opus".to_string()));
        assert!(deserialized.restart);
    }

    #[test]
    fn test_set_model_request_restart_defaults_false() {
        let json = r#"{"model":"sonnet"}"#;
        let request: SetModelRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, Some("sonnet".to_string()));
        assert!(!request.restart);
    }

    #[test]
    fn test_set_model_request_clear_model() {
        let request = SetModelRequest { model: None, restart: false };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":null"));

        let deserialized: SetModelRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, None);
    }

    #[test]
    fn test_tool_policy_mode_str() {
        assert_eq!(ToolPolicy::AllowAll.mode_str(), "allow_all");
        assert_eq!(ToolPolicy::DenyAll.mode_str(), "deny_all");
        assert_eq!(ToolPolicy::RequireApproval.mode_str(), "require_approval");
    }
}
