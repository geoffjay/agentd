use crate::types::ToolPolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// An external task fetched from a task source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier from the source (e.g., issue number).
    pub source_id: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    /// Arbitrary key-value metadata from the source.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Workflow configuration persisted to the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub id: Uuid,
    pub name: String,
    /// The agent that will execute tasks from this workflow.
    pub agent_id: Uuid,
    /// Configuration for the trigger (task source).
    #[serde(alias = "source_config")]
    pub trigger_config: TriggerConfig,
    /// Template string with {{placeholders}} for rendering prompts.
    pub prompt_template: String,
    /// How often to poll the task source, in seconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Whether the workflow is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Tool policy applied to the agent when dispatching tasks from this workflow.
    /// Defaults to AllowAll (no restrictions).
    #[serde(default)]
    pub tool_policy: ToolPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_poll_interval() -> u64 {
    60
}

fn default_enabled() -> bool {
    true
}

/// Tagged enum for different trigger backends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerConfig {
    GithubIssues {
        owner: String,
        repo: String,
        #[serde(default)]
        labels: Vec<String>,
        #[serde(default = "default_issue_state")]
        state: String,
    },
    GithubPullRequests {
        owner: String,
        repo: String,
        #[serde(default)]
        labels: Vec<String>,
        #[serde(default = "default_pr_state")]
        state: String,
    },
    /// Cron-based trigger (Phase 2).
    Cron { expression: String },
    /// One-shot delayed trigger (Phase 2).
    Delay {
        /// ISO 8601 datetime string.
        run_at: String,
    },
    /// Agent lifecycle event trigger (Phase 3).
    ///
    /// Fires when a matching agent lifecycle event occurs on the event bus.
    /// The `event` field maps to system events:
    /// - `"session_start"` → `AgentConnected`
    /// - `"session_end"` → `AgentDisconnected`
    /// - `"context_clear"` → `ContextCleared`
    AgentLifecycle {
        /// The lifecycle event type to listen for.
        event: String,
    },
    /// Dispatch result trigger (Phase 3).
    ///
    /// Fires when a workflow dispatch completes, enabling workflow chaining.
    /// Optionally filter by `source_workflow_id` and/or `status`.
    DispatchResult {
        /// Only trigger when this specific workflow completes.
        #[serde(default)]
        source_workflow_id: Option<Uuid>,
        /// Only trigger on a specific completion status.
        #[serde(default)]
        status: Option<DispatchStatus>,
    },
    /// Webhook-driven trigger (Phase 4).
    Webhook {
        #[serde(default)]
        secret: Option<String>,
    },
    /// Manual trigger — dispatched explicitly via the API.
    Manual {},
    /// Linear issues trigger — polls Linear for issues matching the given filters.
    ///
    /// Requires `AGENTD_LINEAR_API_KEY` to be set in the environment.
    /// All filter fields are optional; omitting them returns all accessible issues.
    ///
    /// # Fields
    ///
    /// - `team_key` — Linear team key to filter by (e.g. `"ENG"`).
    /// - `project` — Linear project name or ID to filter by.
    /// - `status` — Issue status names to include (e.g. `["Todo", "In Progress"]`).
    ///   Defaults to all statuses when omitted.
    /// - `labels` — Label names the issue must have (all must match).
    /// - `assignee` — Assignee display name or email to filter by.
    LinearIssues {
        /// Linear team key filter (e.g. `"ENG"`).
        #[serde(default)]
        team_key: Option<String>,
        /// Linear project name or ID filter.
        #[serde(default)]
        project: Option<String>,
        /// Issue status filter (e.g. `["Todo", "In Progress"]`).
        /// Defaults to all statuses when `None`.
        #[serde(default)]
        status: Option<Vec<String>>,
        /// Label filter — issue must carry all listed labels.
        #[serde(default)]
        labels: Vec<String>,
        /// Assignee display name or email filter.
        #[serde(default)]
        assignee: Option<String>,
    },
}

fn default_issue_state() -> String {
    "open".to_string()
}

fn default_pr_state() -> String {
    "open".to_string()
}

impl TriggerConfig {
    pub fn trigger_type(&self) -> &'static str {
        match self {
            TriggerConfig::GithubIssues { .. } => "github_issues",
            TriggerConfig::GithubPullRequests { .. } => "github_pull_requests",
            TriggerConfig::Cron { .. } => "cron",
            TriggerConfig::Delay { .. } => "delay",
            TriggerConfig::AgentLifecycle { .. } => "agent_lifecycle",
            TriggerConfig::DispatchResult { .. } => "dispatch_result",
            TriggerConfig::Webhook { .. } => "webhook",
            TriggerConfig::Manual { .. } => "manual",
            TriggerConfig::LinearIssues { .. } => "linear_issues",
        }
    }

    /// Returns `true` for trigger types that have a working implementation.
    pub fn is_implemented(&self) -> bool {
        match self {
            TriggerConfig::GithubIssues { .. }
            | TriggerConfig::GithubPullRequests { .. }
            | TriggerConfig::Cron { .. }
            | TriggerConfig::Delay { .. }
            | TriggerConfig::AgentLifecycle { .. }
            | TriggerConfig::DispatchResult { .. }
            | TriggerConfig::Webhook { .. }
            | TriggerConfig::Manual { .. } => true,
            // LinearIssues source is not yet implemented — see issue #475.
            TriggerConfig::LinearIssues { .. } => false,
        }
    }

    /// Returns `true` for one-shot trigger types that should auto-disable
    /// the workflow after firing.
    pub fn is_one_shot(&self) -> bool {
        matches!(self, TriggerConfig::Delay { .. })
    }
}

/// A record tracking each task dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRecord {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub source_id: String,
    pub agent_id: Uuid,
    pub prompt_sent: String,
    pub status: DispatchStatus,
    pub dispatched_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Dispatched,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for DispatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchStatus::Pending => write!(f, "pending"),
            DispatchStatus::Dispatched => write!(f, "dispatched"),
            DispatchStatus::Completed => write!(f, "completed"),
            DispatchStatus::Failed => write!(f, "failed"),
            DispatchStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for DispatchStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(DispatchStatus::Pending),
            "dispatched" => Ok(DispatchStatus::Dispatched),
            "completed" => Ok(DispatchStatus::Completed),
            "failed" => Ok(DispatchStatus::Failed),
            "skipped" => Ok(DispatchStatus::Skipped),
            _ => Err(anyhow::anyhow!("Unknown dispatch status: {}", s)),
        }
    }
}

/// Request body for manually triggering a workflow via the API.
///
/// All fields are optional. Defaults are used when not provided:
/// - `title`: "Manual trigger"
/// - `body`: empty string
/// - `metadata`: empty map
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerWorkflowRequest {
    /// Task title (defaults to "Manual trigger").
    pub title: Option<String>,
    /// Task body / description.
    pub body: Option<String>,
    /// Arbitrary metadata key-value pairs.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Request body for creating a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    pub agent_id: Uuid,
    #[serde(alias = "source_config")]
    pub trigger_config: TriggerConfig,
    pub prompt_template: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Tool policy to apply to the agent when dispatching tasks.
    #[serde(default)]
    pub tool_policy: ToolPolicy,
}

/// Request body for updating a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub prompt_template: Option<String>,
    pub poll_interval_secs: Option<u64>,
    pub enabled: Option<bool>,
    /// Update the tool policy for the workflow's agent.
    pub tool_policy: Option<ToolPolicy>,
}

/// Response body for workflow endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub id: Uuid,
    pub name: String,
    pub agent_id: Uuid,
    #[serde(alias = "source_config")]
    pub trigger_config: TriggerConfig,
    pub prompt_template: String,
    pub poll_interval_secs: u64,
    pub enabled: bool,
    pub tool_policy: ToolPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<WorkflowConfig> for WorkflowResponse {
    fn from(w: WorkflowConfig) -> Self {
        Self {
            id: w.id,
            name: w.name,
            agent_id: w.agent_id,
            trigger_config: w.trigger_config,
            prompt_template: w.prompt_template,
            poll_interval_secs: w.poll_interval_secs,
            enabled: w.enabled,
            tool_policy: w.tool_policy,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

/// Response body for dispatch history entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResponse {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub source_id: String,
    pub agent_id: Uuid,
    pub prompt_sent: String,
    pub status: DispatchStatus,
    pub dispatched_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<DispatchRecord> for DispatchResponse {
    fn from(d: DispatchRecord) -> Self {
        Self {
            id: d.id,
            workflow_id: d.workflow_id,
            source_id: d.source_id,
            agent_id: d.agent_id,
            prompt_sent: d.prompt_sent,
            status: d.status,
            dispatched_at: d.dispatched_at,
            completed_at: d.completed_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_config_linear_issues_trigger_type() {
        let cfg = TriggerConfig::LinearIssues {
            team_key: Some("ENG".into()),
            project: None,
            status: None,
            labels: vec![],
            assignee: None,
        };
        assert_eq!(cfg.trigger_type(), "linear_issues");
    }

    #[test]
    fn test_trigger_config_linear_issues_serde_full() {
        let json = r#"{
            "type": "linear_issues",
            "team_key": "ENG",
            "project": "Backend",
            "status": ["Todo", "In Progress"],
            "labels": ["bug", "urgent"],
            "assignee": "alice@example.com"
        }"#;
        let cfg: TriggerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.trigger_type(), "linear_issues");
        if let TriggerConfig::LinearIssues { team_key, project, status, labels, assignee } = cfg {
            assert_eq!(team_key.as_deref(), Some("ENG"));
            assert_eq!(project.as_deref(), Some("Backend"));
            assert_eq!(
                status.as_deref(),
                Some(&["Todo".to_string(), "In Progress".to_string()][..])
            );
            assert_eq!(labels, vec!["bug", "urgent"]);
            assert_eq!(assignee.as_deref(), Some("alice@example.com"));
        } else {
            panic!("Expected LinearIssues variant");
        }
    }

    #[test]
    fn test_trigger_config_linear_issues_serde_minimal_defaults() {
        // All optional fields omitted — should deserialize with defaults.
        let json = r#"{"type": "linear_issues"}"#;
        let cfg: TriggerConfig = serde_json::from_str(json).unwrap();
        if let TriggerConfig::LinearIssues { team_key, project, status, labels, assignee } = cfg {
            assert!(team_key.is_none());
            assert!(project.is_none());
            assert!(status.is_none());
            assert!(labels.is_empty());
            assert!(assignee.is_none());
        } else {
            panic!("Expected LinearIssues variant");
        }
    }

    #[test]
    fn test_trigger_config_linear_issues_serde_roundtrip() {
        let original = TriggerConfig::LinearIssues {
            team_key: Some("OPS".into()),
            project: Some("Infra".into()),
            status: Some(vec!["Todo".into()]),
            labels: vec!["infra".into()],
            assignee: Some("bob@example.com".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: TriggerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.trigger_type(), "linear_issues");
        // Serialized tag must be snake_case.
        assert!(json.contains(r#""type":"linear_issues""#));
    }
}
