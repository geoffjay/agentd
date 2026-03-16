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
    Cron {
        expression: String,
    },
    /// One-shot delayed trigger (Phase 2).
    Delay {
        /// ISO 8601 datetime string.
        run_at: String,
    },
    /// Webhook-driven trigger (Phase 4).
    Webhook {
        #[serde(default)]
        secret: Option<String>,
    },
    /// Manual trigger — dispatched explicitly via the API.
    Manual {},
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
            TriggerConfig::Webhook { .. } => "webhook",
            TriggerConfig::Manual { .. } => "manual",
        }
    }

    /// Returns `true` for trigger types that use poll-based task fetching.
    pub fn is_poll_based(&self) -> bool {
        matches!(self, TriggerConfig::GithubIssues { .. } | TriggerConfig::GithubPullRequests { .. })
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
