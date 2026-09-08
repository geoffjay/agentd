//! MCP server implementation for agentd.
//!
//! `AgentdMcp` is the central server struct. Its tools are declared in the
//! `#[tool_router]` impl block; `#[tool_handler]` on the `ServerHandler` impl
//! wires them into the MCP `tools/list` and `tools/call` handlers.

use crate::client::AgentdClient;
use crate::config::AgentdMcpConfig;
use crate::tools::{
    agents, approvals, communicate, creation, diagnostic, health, knowledge, lifecycle, memory,
    metrics, notifications, orchestrator_debug, remediation, workflows,
};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ServerHandler,
};
use std::sync::Arc;

/// The agentd MCP server.
///
/// Exposes agentd services (orchestrator, communicate, memory, notify, ask,
/// wrap, and monitor) as MCP tools that Claude and other MCP clients can call.
#[derive(Debug, Clone)]
pub struct AgentdMcp {
    client: AgentdClient,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DiagnoseAgentArgs {
    /// The agent ID (UUID) to diagnose
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DiagnoseWorkflowArgs {
    /// The workflow ID (UUID) to diagnose
    workflow_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct InspectQueueArgs {
    /// Queue name (e.g. agent-tasks, workflow-dispatch)
    queue_name: String,
    /// Number of pending tasks to peek (default: 10, max: 100)
    peek_limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetConversationSummaryArgs {
    /// The agent ID (UUID)
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListProjectsArgs {
    /// Maximum projects to return (default: 50, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetProjectArgs {
    /// The project ID (UUID)
    project_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListAgentsArgs {
    /// Filter by status: pending | running | stopped | failed. Omit for all agents.
    status: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetAgentArgs {
    /// The agent ID (UUID) to inspect
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetWorkflowArgs {
    /// The workflow ID (UUID) to inspect
    workflow_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListDispatchesArgs {
    /// The workflow ID (UUID) to list dispatches for
    workflow_id: String,
    /// Filter by status: pending | dispatched | completed | failed | skipped. Omit for all.
    status: Option<String>,
    /// Maximum number of records to return (default: 20, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetFailedDispatchesArgs {
    /// Maximum number of records to return (default: 50, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateAgentArgs {
    /// Unique agent name
    name: String,
    /// Absolute working directory for the agent process
    working_dir: String,
    /// Model alias or full name (e.g. sonnet, opus, claude-sonnet-4-6). Omit for the default.
    model: Option<String>,
    /// System prompt for the agent's Claude session
    system_prompt: Option<String>,
    /// If true, the system prompt is appended to Claude's default prompt instead of replacing it
    append_system_prompt: Option<bool>,
    /// Initial prompt delivered once the agent connects
    prompt: Option<String>,
    /// Tool policy mode: allow_all | deny_all | require_approval | allow_list | deny_list. Omit for allow_all.
    tool_policy_mode: Option<String>,
    /// Tool names/patterns for allow_list or deny_list modes
    tool_policy_tools: Option<Vec<String>>,
    /// Communicate room names the agent auto-joins on connect
    rooms: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateWorkflowArgs {
    /// Workflow name
    name: String,
    /// ID (UUID) of the agent that receives dispatched prompts — see list_agents
    agent_id: String,
    /// Trigger configuration object with a `type` field (see tool description for examples)
    trigger_config: serde_json::Value,
    /// Prompt template dispatched on trigger; {{placeholder}} variables are substituted from the trigger payload
    prompt_template: String,
    /// Polling interval in seconds for polling triggers (default: 60)
    poll_interval_secs: Option<u64>,
    /// Whether the workflow starts enabled (default: true)
    enabled: Option<bool>,
    /// Tool policy mode applied to dispatched runs: allow_all | deny_all | require_approval | allow_list | deny_list
    tool_policy_mode: Option<String>,
    /// Tool names/patterns for allow_list or deny_list modes
    tool_policy_tools: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateWorkflowArgs {
    /// The workflow ID (UUID) to update
    workflow_id: String,
    /// New workflow name
    name: Option<String>,
    /// Reassign to this agent ID (UUID)
    agent_id: Option<String>,
    /// Replacement trigger configuration object with a `type` field
    trigger_config: Option<serde_json::Value>,
    /// Replacement prompt template
    prompt_template: Option<String>,
    /// New polling interval in seconds
    poll_interval_secs: Option<u64>,
    /// Enable or disable the workflow
    enabled: Option<bool>,
    /// Tool policy mode: allow_all | deny_all | require_approval | allow_list | deny_list
    tool_policy_mode: Option<String>,
    /// Tool names/patterns for allow_list or deny_list modes
    tool_policy_tools: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetWorkflowEnabledArgs {
    /// The workflow ID (UUID)
    workflow_id: String,
    /// true to enable, false to disable
    enabled: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TriggerWorkflowArgs {
    /// The workflow ID (UUID) to trigger
    workflow_id: String,
    /// Task title substituted into {{title}}
    title: Option<String>,
    /// Task body substituted into {{body}}
    body: Option<String>,
    /// Task URL substituted into {{url}}
    url: Option<String>,
    /// Labels substituted into {{labels}}
    labels: Option<Vec<String>>,
    /// Additional string key/value metadata for {{metadata.*}} placeholders
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DeleteWorkflowArgs {
    /// The workflow ID (UUID) to delete
    workflow_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListNotificationsArgs {
    /// Filter by status: pending | viewed | responded | dismissed | expired. Omit for all.
    status: Option<String>,
    /// Filter by priority: low | normal | high | urgent. Omit for all.
    priority: Option<String>,
    /// Maximum number to return (default: 20, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetNotificationArgs {
    /// The notification ID (UUID)
    notification_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateNotificationArgs {
    /// Short title for the notification
    title: String,
    /// Detailed message body with diagnostic context
    message: String,
    /// Priority level: low | normal | high | urgent (default: normal)
    priority: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DismissNotificationArgs {
    /// The notification ID (UUID) to dismiss
    notification_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetAgentApprovalsArgs {
    /// The agent ID (UUID) whose approvals to list
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ApproveToolRequestArgs {
    /// The approval request ID (UUID) to approve
    approval_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DenyToolRequestArgs {
    /// The approval request ID (UUID) to deny
    approval_id: String,
    /// Optional reason for denial, sent back to the agent
    reason: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListRoomsArgs {
    /// Filter by room type: direct | group | broadcast. Omit for all.
    room_type: Option<String>,
    /// Filter by project ID (UUID). Omit for all.
    project_id: Option<String>,
    /// Maximum number of rooms to return (default: 50, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetRoomArgs {
    /// The room ID (UUID) to fetch
    room_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListMessagesArgs {
    /// The room ID (UUID) whose messages to list
    room_id: String,
    /// Maximum number of messages (default: 20, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SendRoomMessageArgs {
    /// The room ID (UUID) to post into
    room_id: String,
    /// Sender identifier (agent ID or human username)
    sender_id: String,
    /// Display name for the sender
    sender_name: String,
    /// Sender kind: agent | human
    sender_kind: String,
    /// The message content to send
    content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RememberArgs {
    /// The natural-language content to store (required, non-empty)
    content: String,
    /// Identity of the actor storing this memory, e.g. your agent name (required)
    created_by: String,
    /// Memory type: information | question | request (default: information)
    memory_type: Option<String>,
    /// Free-form tags for later filtering
    tags: Option<Vec<String>>,
    /// Visibility: public | private | shared (default: public)
    visibility: Option<String>,
    /// Actors allowed to read this memory; required when visibility is `shared`
    shared_with: Option<Vec<String>>,
    /// IDs of related memories this one references
    references: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchMemoriesArgs {
    /// Search query (required, non-empty)
    query: String,
    /// Filter results to memories with all of these tags
    tags: Option<Vec<String>>,
    /// Filter by memory type: information | question | request
    memory_type: Option<String>,
    /// Maximum results (default: 10, max: 100)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ListMemoriesArgs {
    /// Filter by memory type: information | question | request
    memory_type: Option<String>,
    /// Filter by a tag (single tag)
    tag: Option<String>,
    /// Filter by creator identifier
    created_by: Option<String>,
    /// Filter by visibility: public | private | shared
    visibility: Option<String>,
    /// Maximum results (default: 50, max: 200)
    limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetMemoryArgs {
    /// The memory ID (e.g. mem_1718000000000_a1b2c3d4)
    memory_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeListDocumentsArgs {
    /// Project UUID that owns the documents
    project_id: String,
    /// Only return documents whose rel_path starts with this prefix, e.g. `docs/`
    prefix: Option<String>,
    /// Maximum results (default: 50, max: 500)
    limit: Option<u32>,
    /// Pagination offset (default: 0)
    offset: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeReadDocumentArgs {
    /// Project UUID that owns the document
    project_id: String,
    /// Document UUID to read
    doc_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeCreateDocumentArgs {
    /// Project UUID to create the document under
    project_id: String,
    /// Relative path ending in `.md`, e.g. `docs/readme.md` (no `..`, no absolute paths)
    rel_path: String,
    /// Initial markdown body content
    content: String,
    /// Optional title override (defaults to the filename stem)
    title: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeUpdateDocumentArgs {
    /// Project UUID that owns the document
    project_id: String,
    /// Document UUID to update
    doc_id: String,
    /// New markdown body content (replaces the whole body)
    content: Option<String>,
    /// New title
    title: Option<String>,
    /// Optimistic concurrency token: the RFC3339 updated_at from a prior read
    expected_updated_at: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeDeleteDocumentArgs {
    /// Project UUID that owns the document
    project_id: String,
    /// Document UUID to delete
    doc_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct KnowledgeGetTreeArgs {
    /// Project UUID whose document tree to render
    project_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RestartAgentArgs {
    /// The agent ID (UUID) to restart
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SendAgentMessageArgs {
    /// The agent ID (UUID) to message
    agent_id: String,
    /// The message content or prompt to send to the agent
    message: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateAgentToolPolicyArgs {
    /// The agent ID (UUID)
    agent_id: String,
    /// Policy mode: allow_all | deny_all | require_approval | allow_list | deny_list
    mode: String,
    /// Tool name patterns for allow_list or deny_list modes (e.g. ["Bash", "Write"])
    tools: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TerminateAgentArgs {
    /// The agent ID (UUID) to terminate
    agent_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateAgentModelArgs {
    /// The agent ID (UUID)
    agent_id: String,
    /// The new model identifier (e.g. claude-opus-4-5, claude-sonnet-4-5)
    model: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RetryFailedDispatchesArgs {
    /// The workflow ID (UUID) to retry dispatches for
    workflow_id: String,
    /// Only retry dispatches that failed within this many hours (default: 24)
    hours: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CleanupStaleDispatchesArgs {
    /// Consider dispatches stale after this many hours (default: 2)
    stale_hours: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AutoApproveSafeToolsArgs {
    /// Additional tool names to consider safe beyond the default read-only set (e.g. ["LSP", "TaskOutput"])
    additional_safe_tools: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ResolveNotificationBacklogArgs {
    /// Dismiss low-priority notifications older than this many hours (default: 48)
    hours: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CheckSingleServiceArgs {
    /// Service name: orchestrator | communicate | memory | notify | ask | wrap | monitor | hook
    service: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetPrometheusMetricsArgs {
    /// Service: orchestrator | notify | memory | communicate | monitor | ask | wrap | hook (default: orchestrator)
    service: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct QueryMetricsArgs {
    /// Catalog query name; omit to list the catalog
    name: Option<String>,
    /// Time window for rate/increase queries, e.g. 15m, 1h, 24h
    window: Option<String>,
    /// true for a range query (trend over the last 6h) instead of an instant value
    range: Option<bool>,
}

#[tool_router]
impl AgentdMcp {
    /// Create a new server instance from the given configuration.
    pub fn new(config: AgentdMcpConfig) -> Self {
        let config = Arc::new(config);
        Self { client: AgentdClient::new(config) }
    }

    /// Run a comprehensive diagnostic on an agent: check status, activity,
    /// pending approval backlog, and usage. Returns a markdown report with
    /// identified issues and suggested fixes referencing other MCP tools.
    #[tool(
        description = "Run a comprehensive diagnostic on an agent: check status, activity state, WebSocket connection, pending approval backlog, and session usage. Returns a structured report with severity-tagged issues and actionable remediation steps."
    )]
    async fn diagnose_agent(
        &self,
        Parameters(diagnose_agent_args): Parameters<DiagnoseAgentArgs>,
    ) -> String {
        let DiagnoseAgentArgs { agent_id } = diagnose_agent_args;
        diagnostic::run_diagnose_agent(&self.client, &agent_id).await
    }

    /// Analyse a workflow's configuration, associated agent health, and
    /// dispatch history. Identifies failure patterns and success rate issues.
    #[tool(
        description = "Run a diagnostic on a workflow: verify the associated agent is running, analyze dispatch success rate over the last 20 dispatches, and identify consecutive failure patterns. Returns a markdown report with severity-tagged issues."
    )]
    async fn diagnose_workflow(
        &self,
        Parameters(diagnose_workflow_args): Parameters<DiagnoseWorkflowArgs>,
    ) -> String {
        let DiagnoseWorkflowArgs { workflow_id } = diagnose_workflow_args;
        diagnostic::run_diagnose_workflow(&self.client, &workflow_id).await
    }

    /// Full system health overview. Checks all services, counts failed agents,
    /// surfaces monitor alerts, and reports notification/approval backlogs.
    #[tool(
        description = "Run a full system diagnostic: check all agentd services, identify failed agents, surface monitor alerts, count pending approvals and notifications. Returns a prioritized report: 🔴 Critical / 🟡 Warning / 🟢 Info. Tolerates partial service unavailability."
    )]
    async fn diagnose_system(&self) -> String {
        diagnostic::run_diagnose_system(&self.client).await
    }

    /// Detect orchestrator state mismatches (running-but-disconnected, etc.).
    #[tool(
        description = "Detect orchestrator state mismatches: agents that are running but have no WebSocket connection, agents connected but not in running state, and orphan WebSocket connections with no DB record. Highest-value tool for catching subtle agent stuckness."
    )]
    async fn diagnose_state_mismatches(&self) -> String {
        orchestrator_debug::run_diagnose_state_mismatches(&self.client).await
    }

    /// Inspect a queue: stats and a peek at the next pending tasks.
    #[tool(
        description = "Inspect a named queue managed by the orchestrator. Returns counts (pending, processing, completed, failed, dead) and peeks at the next N pending tasks with their retry counts. Useful for diagnosing queue backpressure or stuck workflow triggers."
    )]
    async fn inspect_queue(
        &self,
        Parameters(inspect_queue_args): Parameters<InspectQueueArgs>,
    ) -> String {
        let InspectQueueArgs { queue_name, peek_limit } = inspect_queue_args;
        orchestrator_debug::run_inspect_queue(&self.client, &queue_name, peek_limit).await
    }

    /// Summarise an agent's conversation history (event counts, sessions, time range).
    #[tool(
        description = "Get a summary of an agent's conversation history: total events, per-event-type counts, session count, first/last event timestamps. Cheap way to see whether an agent has actually been productive without loading full transcripts."
    )]
    async fn get_conversation_summary(
        &self,
        Parameters(get_conversation_summary_args): Parameters<GetConversationSummaryArgs>,
    ) -> String {
        let GetConversationSummaryArgs { agent_id } = get_conversation_summary_args;
        orchestrator_debug::run_get_conversation_summary(&self.client, &agent_id).await
    }

    /// List projects (groupings of agents and workflows).
    #[tool(
        description = "List projects defined in the orchestrator. Projects group agents and workflows for organisational purposes."
    )]
    async fn list_projects(
        &self,
        Parameters(list_projects_args): Parameters<ListProjectsArgs>,
    ) -> String {
        let ListProjectsArgs { limit } = list_projects_args;
        orchestrator_debug::run_list_projects(&self.client, limit).await
    }

    /// Get a project's details including agent and workflow counts.
    #[tool(
        description = "Get a project's details including counts of associated agents and workflows."
    )]
    async fn get_project(
        &self,
        Parameters(get_project_args): Parameters<GetProjectArgs>,
    ) -> String {
        let GetProjectArgs { project_id } = get_project_args;
        orchestrator_debug::run_get_project(&self.client, &project_id).await
    }

    // ── Agent inspection ────────────────────────────────────────────────

    /// List all agents managed by agentd.
    #[tool(
        description = "List all agents managed by agentd, optionally filtered by status. Returns a table with agent ID, name, status, and activity state."
    )]
    async fn list_agents(
        &self,
        Parameters(list_agents_args): Parameters<ListAgentsArgs>,
    ) -> String {
        let ListAgentsArgs { status } = list_agents_args;
        agents::run_list_agents(&self.client, status.as_deref()).await
    }

    /// Get full details for a specific agent.
    #[tool(
        description = "Get detailed information about a specific agent including its configuration, tool policy, model, working directory, and environment variable keys (values are redacted)."
    )]
    async fn get_agent(&self, Parameters(get_agent_args): Parameters<GetAgentArgs>) -> String {
        let GetAgentArgs { agent_id } = get_agent_args;
        agents::run_get_agent(&self.client, &agent_id).await
    }

    /// Get a fleet-wide summary of agent statuses.
    #[tool(
        description = "Get a summary of all agent statuses: counts of pending, running, stopped, and failed agents. Lists any failed agents with their IDs and names for quick identification."
    )]
    async fn get_agent_status_summary(&self) -> String {
        agents::run_get_agent_status_summary(&self.client).await
    }

    // ── Workflow and dispatch inspection ────────────────────────────────

    /// List all configured workflows with status and associated agent.
    #[tool(
        description = "List all configured workflows with their trigger type, poll interval, enabled state, and associated agent. Use get_workflow for full details including prompt template and source config."
    )]
    async fn list_workflows(&self) -> String {
        workflows::run_list_workflows(&self.client).await
    }

    /// Get full configuration of a workflow.
    #[tool(
        description = "Get full configuration of a workflow including trigger source config, prompt template, and tool policy. Useful for understanding what a workflow dispatches and how it is configured."
    )]
    async fn get_workflow(
        &self,
        Parameters(get_workflow_args): Parameters<GetWorkflowArgs>,
    ) -> String {
        let GetWorkflowArgs { workflow_id } = get_workflow_args;
        workflows::run_get_workflow(&self.client, &workflow_id).await
    }

    /// List dispatch records for a workflow.
    #[tool(
        description = "List dispatch records for a specific workflow showing task execution history with status and timing. Supports optional status filter and limit."
    )]
    async fn list_dispatches(
        &self,
        Parameters(list_dispatches_args): Parameters<ListDispatchesArgs>,
    ) -> String {
        let ListDispatchesArgs { workflow_id, status, limit } = list_dispatches_args;
        workflows::run_list_dispatches(&self.client, &workflow_id, status.as_deref(), limit).await
    }

    /// Get all failed dispatch records across all workflows.
    #[tool(
        description = "Get all failed dispatch records across all workflows, useful for identifying systemic issues. Returns failures sorted by most recent first."
    )]
    async fn get_failed_dispatches(
        &self,
        Parameters(get_failed_dispatches_args): Parameters<GetFailedDispatchesArgs>,
    ) -> String {
        let GetFailedDispatchesArgs { limit } = get_failed_dispatches_args;
        workflows::run_get_failed_dispatches(&self.client, limit).await
    }

    // ── Agent and workflow creation ─────────────────────────────────────

    /// Create a new agent.
    #[tool(
        description = "Create a new agent. Exposes the safe subset of agent configuration: name, working_dir, model, system_prompt, initial prompt, tool policy, and rooms. Env vars and Docker settings are deliberately NOT exposed (secrets must not transit MCP) — use the CLI for those. Returns the new agent ID."
    )]
    async fn create_agent(
        &self,
        Parameters(create_agent_args): Parameters<CreateAgentArgs>,
    ) -> String {
        let CreateAgentArgs {
            name,
            working_dir,
            model,
            system_prompt,
            append_system_prompt,
            prompt,
            tool_policy_mode,
            tool_policy_tools,
            rooms,
        } = create_agent_args;
        creation::run_create_agent(
            &self.client,
            &name,
            &working_dir,
            model.as_deref(),
            system_prompt.as_deref(),
            append_system_prompt.unwrap_or(false),
            prompt.as_deref(),
            tool_policy_mode.as_deref(),
            tool_policy_tools,
            rooms,
        )
        .await
    }

    /// Create a new workflow.
    #[tool(
        description = "Create a new workflow that dispatches prompts to an agent when its trigger fires. trigger_config is a JSON object with a `type` field, e.g. {\"type\":\"cron\",\"expression\":\"0 9 * * MON-FRI\"}, {\"type\":\"manual\"}, or {\"type\":\"agent_idle\",\"idle_seconds\":600}. Valid types: github_issues, github_pull_requests, cron, delay, agent_lifecycle, dispatch_result, webhook, manual, agent_idle, linear_issues, composite, queue, ask_response, gitlab_issues, gitlab_merge_requests. Use get_workflow on an existing workflow to copy a working config. prompt_template supports {{placeholder}} substitution and is validated server-side."
    )]
    async fn create_workflow(
        &self,
        Parameters(create_workflow_args): Parameters<CreateWorkflowArgs>,
    ) -> String {
        let CreateWorkflowArgs {
            name,
            agent_id,
            trigger_config,
            prompt_template,
            poll_interval_secs,
            enabled,
            tool_policy_mode,
            tool_policy_tools,
        } = create_workflow_args;
        creation::run_create_workflow(
            &self.client,
            &name,
            &agent_id,
            trigger_config,
            &prompt_template,
            poll_interval_secs,
            enabled,
            tool_policy_mode.as_deref(),
            tool_policy_tools,
        )
        .await
    }

    /// Update an existing workflow.
    #[tool(
        description = "Update an existing workflow. Only the provided fields change (merge semantics): name, agent_id, trigger_config, prompt_template, poll_interval_secs, enabled, tool policy. Use set_workflow_enabled for a plain enable/disable toggle."
    )]
    async fn update_workflow(
        &self,
        Parameters(update_workflow_args): Parameters<UpdateWorkflowArgs>,
    ) -> String {
        let UpdateWorkflowArgs {
            workflow_id,
            name,
            agent_id,
            trigger_config,
            prompt_template,
            poll_interval_secs,
            enabled,
            tool_policy_mode,
            tool_policy_tools,
        } = update_workflow_args;
        creation::run_update_workflow(
            &self.client,
            &workflow_id,
            name.as_deref(),
            agent_id.as_deref(),
            trigger_config,
            prompt_template.as_deref(),
            poll_interval_secs,
            enabled,
            tool_policy_mode.as_deref(),
            tool_policy_tools,
        )
        .await
    }

    /// Enable or disable a workflow.
    #[tool(
        description = "Enable or disable a workflow. Disabled workflows keep their configuration and history but their trigger no longer fires."
    )]
    async fn set_workflow_enabled(
        &self,
        Parameters(set_workflow_enabled_args): Parameters<SetWorkflowEnabledArgs>,
    ) -> String {
        let SetWorkflowEnabledArgs { workflow_id, enabled } = set_workflow_enabled_args;
        creation::run_set_workflow_enabled(&self.client, &workflow_id, enabled).await
    }

    /// Manually trigger a workflow.
    #[tool(
        description = "Manually trigger a workflow, dispatching its prompt template immediately. Optional title/body/url/labels/metadata populate the {{placeholder}} variables in the template. Ideal for smoke-testing a freshly created workflow."
    )]
    async fn trigger_workflow(
        &self,
        Parameters(trigger_workflow_args): Parameters<TriggerWorkflowArgs>,
    ) -> String {
        let TriggerWorkflowArgs { workflow_id, title, body, url, labels, metadata } =
            trigger_workflow_args;
        creation::run_trigger_workflow(
            &self.client,
            &workflow_id,
            title.as_deref(),
            body.as_deref(),
            url.as_deref(),
            labels,
            metadata,
        )
        .await
    }

    /// Delete a workflow.
    #[tool(
        description = "⚠️ DESTRUCTIVE: Delete a workflow permanently. Its trigger stops firing and the configuration is removed (dispatch history is retained). Prefer set_workflow_enabled(false) when you may need the workflow again."
    )]
    async fn delete_workflow(
        &self,
        Parameters(delete_workflow_args): Parameters<DeleteWorkflowArgs>,
    ) -> String {
        let DeleteWorkflowArgs { workflow_id } = delete_workflow_args;
        creation::run_delete_workflow(&self.client, &workflow_id).await
    }

    // ── Notification management ─────────────────────────────────────────

    /// List notifications with optional filters.
    #[tool(
        description = "List notifications with optional filters for status and priority. Sorted by priority (highest first). Use get_actionable_notifications for a focused view of items requiring attention."
    )]
    async fn list_notifications(
        &self,
        Parameters(list_notifications_args): Parameters<ListNotificationsArgs>,
    ) -> String {
        let ListNotificationsArgs { status, priority, limit } = list_notifications_args;
        notifications::run_list_notifications(
            &self.client,
            status.as_deref(),
            priority.as_deref(),
            limit,
        )
        .await
    }

    /// Get full details of a specific notification.
    #[tool(
        description = "Get full details of a specific notification including source data, message body, and response (if any)."
    )]
    async fn get_notification(
        &self,
        Parameters(get_notification_args): Parameters<GetNotificationArgs>,
    ) -> String {
        let GetNotificationArgs { notification_id } = get_notification_args;
        notifications::run_get_notification(&self.client, &notification_id).await
    }

    /// Get all actionable notifications requiring a response.
    #[tool(
        description = "Get all notifications that are pending or viewed and have not expired. These require attention or a response. Sorted by priority (urgent first)."
    )]
    async fn get_actionable_notifications(&self) -> String {
        notifications::run_get_actionable_notifications(&self.client).await
    }

    /// Create a system notification.
    #[tool(
        description = "Create a system notification, useful for flagging issues found during diagnostics or remediation workflows. Creates a persistent, non-response-required notification."
    )]
    async fn create_notification(
        &self,
        Parameters(create_notification_args): Parameters<CreateNotificationArgs>,
    ) -> String {
        let CreateNotificationArgs { title, message, priority } = create_notification_args;
        notifications::run_create_notification(&self.client, &title, &message, priority.as_deref())
            .await
    }

    /// Dismiss a notification.
    #[tool(
        description = "Dismiss a notification, marking it as reviewed and removing it from the active backlog."
    )]
    async fn dismiss_notification(
        &self,
        Parameters(dismiss_notification_args): Parameters<DismissNotificationArgs>,
    ) -> String {
        let DismissNotificationArgs { notification_id } = dismiss_notification_args;
        notifications::run_dismiss_notification(&self.client, &notification_id).await
    }

    // ── Approval management ─────────────────────────────────────────────

    /// List all pending tool approval requests across all agents.
    #[tool(
        description = "List all pending tool approval requests across all agents. Shows tool name, input summary, requesting agent, and expiry time. Use approve_tool_request or deny_tool_request to action items."
    )]
    async fn list_pending_approvals(&self) -> String {
        approvals::run_list_pending_approvals(&self.client).await
    }

    /// List pending tool approval requests for a specific agent.
    #[tool(
        description = "List pending tool approval requests for a specific agent. Useful when diagnosing why an agent appears blocked."
    )]
    async fn get_agent_approvals(
        &self,
        Parameters(get_agent_approvals_args): Parameters<GetAgentApprovalsArgs>,
    ) -> String {
        let GetAgentApprovalsArgs { agent_id } = get_agent_approvals_args;
        approvals::run_get_agent_approvals(&self.client, &agent_id).await
    }

    /// Approve a pending tool use request, allowing the agent to proceed.
    #[tool(
        description = "Approve a pending tool use request, allowing the agent to proceed with the tool invocation. The agent will resume execution once approved."
    )]
    async fn approve_tool_request(
        &self,
        Parameters(approve_tool_request_args): Parameters<ApproveToolRequestArgs>,
    ) -> String {
        let ApproveToolRequestArgs { approval_id } = approve_tool_request_args;
        approvals::run_approve_tool_request(&self.client, &approval_id).await
    }

    /// Deny a pending tool use request with an optional reason.
    #[tool(
        description = "Deny a pending tool use request, preventing the agent from using the tool. An optional reason is sent back to the agent."
    )]
    async fn deny_tool_request(
        &self,
        Parameters(deny_tool_request_args): Parameters<DenyToolRequestArgs>,
    ) -> String {
        let DenyToolRequestArgs { approval_id, reason } = deny_tool_request_args;
        approvals::run_deny_tool_request(&self.client, &approval_id, reason.as_deref()).await
    }

    // ── Communicate: rooms, participants, messages ──────────────────────

    /// List rooms in the communicate service.
    #[tool(
        description = "List rooms in the communicate service. Optionally filter by room_type (direct | group | broadcast) or project_id. Returns a table with type, name, ID, topic, and creation time."
    )]
    async fn list_rooms(&self, Parameters(list_rooms_args): Parameters<ListRoomsArgs>) -> String {
        let ListRoomsArgs { room_type, project_id, limit } = list_rooms_args;
        communicate::run_list_rooms(
            &self.client,
            room_type.as_deref(),
            project_id.as_deref(),
            limit,
        )
        .await
    }

    /// Get a room's details plus its participant list.
    #[tool(
        description = "Get a room's metadata and full participant list. Useful for understanding who's in a room and what its purpose is."
    )]
    async fn get_room(&self, Parameters(get_room_args): Parameters<GetRoomArgs>) -> String {
        let GetRoomArgs { room_id } = get_room_args;
        communicate::run_get_room(&self.client, &room_id).await
    }

    /// List recent messages in a room.
    #[tool(
        description = "List the most recent messages in a room. Returns a table with timestamp, sender, status, and content preview. Useful for catching up on conversation activity or diagnosing what an agent has been saying."
    )]
    async fn list_messages(
        &self,
        Parameters(list_messages_args): Parameters<ListMessagesArgs>,
    ) -> String {
        let ListMessagesArgs { room_id, limit } = list_messages_args;
        communicate::run_list_messages(&self.client, &room_id, limit).await
    }

    /// Post a message into a room as a given sender.
    #[tool(
        description = "Send a message to a room. The sender must be a participant in the room. Useful for remediation flows where the system or an admin agent needs to post a status update or coordinate with other agents."
    )]
    async fn send_room_message(
        &self,
        Parameters(send_room_message_args): Parameters<SendRoomMessageArgs>,
    ) -> String {
        let SendRoomMessageArgs { room_id, sender_id, sender_name, sender_kind, content } =
            send_room_message_args;
        communicate::run_send_room_message(
            &self.client,
            &room_id,
            &sender_id,
            &sender_name,
            &sender_kind,
            &content,
        )
        .await
    }

    // ── Memory: remember, search, list, get ─────────────────────────────

    /// Store a new memory.
    #[tool(
        description = "Store a new memory in the shared agent memory service. The content is embedded for later semantic search. Use this to persist a fact, decision, open question, or request so it survives across sessions and is discoverable by other agents. `created_by` should be your own agent identity. Defaults: type=information, visibility=public."
    )]
    async fn remember(&self, Parameters(remember_args): Parameters<RememberArgs>) -> String {
        let RememberArgs {
            content,
            created_by,
            memory_type,
            tags,
            visibility,
            shared_with,
            references,
        } = remember_args;
        memory::run_create_memory(
            &self.client,
            &content,
            &created_by,
            memory_type.as_deref(),
            tags,
            visibility.as_deref(),
            shared_with,
            references,
        )
        .await
    }

    /// Semantic search across stored memories.
    #[tool(
        description = "Semantic search across agent memories. Returns matching memories ranked by relevance. Useful for discovering what knowledge agents have stored, finding context for a diagnostic, or surfacing relevant prior decisions."
    )]
    async fn search_memories(
        &self,
        Parameters(search_memories_args): Parameters<SearchMemoriesArgs>,
    ) -> String {
        let SearchMemoriesArgs { query, tags, memory_type, limit } = search_memories_args;
        memory::run_search_memories(&self.client, &query, tags, memory_type.as_deref(), limit).await
    }

    /// List memories with optional filters.
    #[tool(
        description = "List memories with optional filters for type, tag, creator, and visibility. Use search_memories for content-based discovery and this tool for metadata-based browsing."
    )]
    async fn list_memories(
        &self,
        Parameters(list_memories_args): Parameters<ListMemoriesArgs>,
    ) -> String {
        let ListMemoriesArgs { memory_type, tag, created_by, visibility, limit } =
            list_memories_args;
        memory::run_list_memories(
            &self.client,
            memory_type.as_deref(),
            tag.as_deref(),
            created_by.as_deref(),
            visibility.as_deref(),
            limit,
        )
        .await
    }

    /// Get full details of a single memory by ID.
    #[tool(
        description = "Fetch a single memory by its ID, including full content, tags, visibility, references, and sharing list."
    )]
    async fn get_memory(&self, Parameters(get_memory_args): Parameters<GetMemoryArgs>) -> String {
        let GetMemoryArgs { memory_id } = get_memory_args;
        memory::run_get_memory(&self.client, &memory_id).await
    }

    // ── Knowledge: per-project markdown documents ───────────────────────

    /// List documents in a project's knowledgebase.
    #[tool(
        description = "List markdown documents in a project's knowledgebase. Returns a table of document IDs, paths, titles, sizes, and last-updated times. Supports a path prefix filter (e.g. `docs/`) and pagination. Use knowledge_get_tree for a hierarchical view, or knowledge_read_document to fetch a document's content."
    )]
    async fn knowledge_list_documents(
        &self,
        Parameters(knowledge_list_documents_args): Parameters<KnowledgeListDocumentsArgs>,
    ) -> String {
        let KnowledgeListDocumentsArgs { project_id, prefix, limit, offset } =
            knowledge_list_documents_args;
        knowledge::run_list_documents(&self.client, &project_id, prefix.as_deref(), limit, offset)
            .await
    }

    /// Read a document's metadata and full markdown body.
    #[tool(
        description = "Read a single knowledgebase document's metadata and full markdown body by its ID. Use knowledge_list_documents or knowledge_get_tree first to discover document IDs."
    )]
    async fn knowledge_read_document(
        &self,
        Parameters(knowledge_read_document_args): Parameters<KnowledgeReadDocumentArgs>,
    ) -> String {
        let KnowledgeReadDocumentArgs { project_id, doc_id } = knowledge_read_document_args;
        knowledge::run_read_document(&self.client, &project_id, &doc_id).await
    }

    /// Create a new document in a project's knowledgebase.
    #[tool(
        description = "Create a new markdown document in a project's knowledgebase. `rel_path` must end in `.md` and contain no `..` or absolute path segments (e.g. `docs/design/auth.md`). Title defaults to the filename stem when omitted. Returns the created document's metadata including its new ID."
    )]
    async fn knowledge_create_document(
        &self,
        Parameters(knowledge_create_document_args): Parameters<KnowledgeCreateDocumentArgs>,
    ) -> String {
        let KnowledgeCreateDocumentArgs { project_id, rel_path, content, title } =
            knowledge_create_document_args;
        knowledge::run_create_document(
            &self.client,
            &project_id,
            &rel_path,
            &content,
            title.as_deref(),
        )
        .await
    }

    /// Update a document's content and/or title.
    #[tool(
        description = "Update an existing knowledgebase document's content and/or title. Provide at least one of `content` or `title`. For safe concurrent edits, pass `expected_updated_at` (the RFC3339 updated_at from a prior read); the update is rejected with a conflict if the document changed since then."
    )]
    async fn knowledge_update_document(
        &self,
        Parameters(knowledge_update_document_args): Parameters<KnowledgeUpdateDocumentArgs>,
    ) -> String {
        let KnowledgeUpdateDocumentArgs { project_id, doc_id, content, title, expected_updated_at } =
            knowledge_update_document_args;
        knowledge::run_update_document(
            &self.client,
            &project_id,
            &doc_id,
            content.as_deref(),
            title.as_deref(),
            expected_updated_at.as_deref(),
        )
        .await
    }

    /// Delete a document from a project's knowledgebase.
    #[tool(
        description = "⚠️ DESTRUCTIVE — Permanently delete a knowledgebase document by ID, removing both its database row and the markdown file on disk. This cannot be undone."
    )]
    async fn knowledge_delete_document(
        &self,
        Parameters(knowledge_delete_document_args): Parameters<KnowledgeDeleteDocumentArgs>,
    ) -> String {
        let KnowledgeDeleteDocumentArgs { project_id, doc_id } = knowledge_delete_document_args;
        knowledge::run_delete_document(&self.client, &project_id, &doc_id).await
    }

    /// Show the virtual folder/file tree for a project's knowledgebase.
    #[tool(
        description = "Show the virtual folder/file tree for a project's knowledgebase, derived from document rel_paths. Useful for understanding the document layout at a glance before listing or reading."
    )]
    async fn knowledge_get_tree(
        &self,
        Parameters(knowledge_get_tree_args): Parameters<KnowledgeGetTreeArgs>,
    ) -> String {
        let KnowledgeGetTreeArgs { project_id } = knowledge_get_tree_args;
        knowledge::run_get_tree(&self.client, &project_id).await
    }

    // ── Agent lifecycle management ──────────────────────────────────────

    /// Restart a failed or stopped agent by terminating and recreating it.
    ///
    /// ⚠️ DESTRUCTIVE: kills the current tmux session and loses in-flight work.
    #[tool(
        description = "⚠️ DESTRUCTIVE — Restart an agent by terminating the current session and recreating it with the same configuration. Loses all in-flight work. Use on Failed or Stopped agents. Returns the new agent ID."
    )]
    async fn restart_agent(
        &self,
        Parameters(restart_agent_args): Parameters<RestartAgentArgs>,
    ) -> String {
        let RestartAgentArgs { agent_id } = restart_agent_args;
        lifecycle::run_restart_agent(&self.client, &agent_id).await
    }

    /// Send a message/prompt to a running agent.
    #[tool(
        description = "Send a message or prompt to a running agent via the orchestrator. The agent will process the message in its current session context."
    )]
    async fn send_agent_message(
        &self,
        Parameters(send_agent_message_args): Parameters<SendAgentMessageArgs>,
    ) -> String {
        let SendAgentMessageArgs { agent_id, message } = send_agent_message_args;
        lifecycle::run_send_agent_message(&self.client, &agent_id, &message).await
    }

    /// Update an agent's tool policy.
    #[tool(
        description = "Update an agent's tool policy to restrict or allow tool usage. Modes: allow_all, deny_all, require_approval, allow_list (needs tools), deny_list (needs tools)."
    )]
    async fn update_agent_tool_policy(
        &self,
        Parameters(update_agent_tool_policy_args): Parameters<UpdateAgentToolPolicyArgs>,
    ) -> String {
        let UpdateAgentToolPolicyArgs { agent_id, mode, tools } = update_agent_tool_policy_args;
        lifecycle::run_update_agent_tool_policy(&self.client, &agent_id, &mode, tools).await
    }

    /// Terminate a running agent — kills the tmux session permanently.
    ///
    /// ⚠️ DESTRUCTIVE: all in-flight work is lost and cannot be recovered.
    #[tool(
        description = "⚠️ DESTRUCTIVE — Terminate a running agent. Kills the tmux session and removes the agent from the registry. All in-flight work is permanently lost. Prefer restart_agent if you intend to recover the agent."
    )]
    async fn terminate_agent(
        &self,
        Parameters(terminate_agent_args): Parameters<TerminateAgentArgs>,
    ) -> String {
        let TerminateAgentArgs { agent_id } = terminate_agent_args;
        lifecycle::run_terminate_agent(&self.client, &agent_id).await
    }

    /// Change the model an agent is using.
    #[tool(
        description = "Change the AI model an agent is using (e.g. switch from claude-sonnet to claude-opus). Takes effect for subsequent turns in the current session."
    )]
    async fn update_agent_model(
        &self,
        Parameters(update_agent_model_args): Parameters<UpdateAgentModelArgs>,
    ) -> String {
        let UpdateAgentModelArgs { agent_id, model } = update_agent_model_args;
        lifecycle::run_update_agent_model(&self.client, &agent_id, &model).await
    }

    // ── Self-healing remediation ────────────────────────────────────────

    /// Find all failed agents and restart them.
    #[tool(
        description = "⚠️ DESTRUCTIVE — Find all agents in a failed state and restart them. For each: captures config, terminates the old session, and recreates the agent. Returns a report of successes and failures. Only targets agents already in failed state."
    )]
    async fn restart_failed_agents(&self) -> String {
        remediation::run_restart_failed_agents(&self.client).await
    }

    /// Retry failed dispatches for a workflow within a time window.
    #[tool(
        description = "Retry failed dispatch records for a workflow by re-sending their prompts to the associated agent. Only retries dispatches that failed within the given time window."
    )]
    async fn retry_failed_dispatches(
        &self,
        Parameters(retry_failed_dispatches_args): Parameters<RetryFailedDispatchesArgs>,
    ) -> String {
        let RetryFailedDispatchesArgs { workflow_id, hours } = retry_failed_dispatches_args;
        remediation::run_retry_failed_dispatches(&self.client, &workflow_id, hours).await
    }

    /// Identify dispatches stuck in 'dispatched' state beyond the staleness threshold.
    #[tool(
        description = "Identify dispatch records stuck in 'dispatched' state longer than the staleness threshold. Reports stale dispatches for visibility — use restart_agent on the associated agent to unblock. Note: the orchestrator API does not support direct dispatch status updates."
    )]
    async fn cleanup_stale_dispatches(
        &self,
        Parameters(cleanup_stale_dispatches_args): Parameters<CleanupStaleDispatchesArgs>,
    ) -> String {
        let CleanupStaleDispatchesArgs { stale_hours } = cleanup_stale_dispatches_args;
        remediation::run_cleanup_stale_dispatches(&self.client, stale_hours).await
    }

    /// Auto-approve pending tool requests matching the safe list.
    #[tool(
        description = "Automatically approve pending tool requests that match a conservative safe list of read-only tools (Read, Glob, Grep, ListFiles, WebFetch, etc.). Non-matching requests are skipped and reported. Additional tools can be added to the safe list via the parameter."
    )]
    async fn auto_approve_safe_tools(
        &self,
        Parameters(auto_approve_safe_tools_args): Parameters<AutoApproveSafeToolsArgs>,
    ) -> String {
        let AutoApproveSafeToolsArgs { additional_safe_tools } = auto_approve_safe_tools_args;
        remediation::run_auto_approve_safe_tools(&self.client, additional_safe_tools).await
    }

    /// Bulk-dismiss old notifications that are no longer actionable.
    #[tool(
        description = "Analyze and bulk-dismiss pending notifications that are no longer actionable: expired ephemeral notifications and low-priority notifications older than the threshold. Returns an audit report of dismissed vs retained items."
    )]
    async fn resolve_notification_backlog(
        &self,
        Parameters(resolve_notification_backlog_args): Parameters<ResolveNotificationBacklogArgs>,
    ) -> String {
        let ResolveNotificationBacklogArgs { hours } = resolve_notification_backlog_args;
        remediation::run_resolve_notification_backlog(&self.client, hours).await
    }

    // ── Service health and system metrics ───────────────────────────────

    /// Concurrently check health of all agentd services.
    #[tool(
        description = "Check the health of all agentd services (orchestrator, communicate, memory, notify, ask, wrap, monitor, hook) concurrently. Returns a table with status, response time, and URL for each. Uses a 3-second timeout per service."
    )]
    async fn check_service_health(&self) -> String {
        health::run_check_service_health(&self.client).await
    }

    /// Check health of a single named service.
    #[tool(
        description = "Check health of a specific agentd service by name. Valid names: orchestrator, communicate, memory, notify, ask, wrap, monitor, hook."
    )]
    async fn check_single_service(
        &self,
        Parameters(check_single_service_args): Parameters<CheckSingleServiceArgs>,
    ) -> String {
        let CheckSingleServiceArgs { service } = check_single_service_args;
        health::run_check_single_service(&self.client, &service).await
    }

    /// Get current system metrics from the monitor service.
    #[tool(
        description = "Get current system metrics from the monitor service: CPU usage, memory, disk usage, and load average. Includes active alerts if any thresholds are exceeded. Returns a degraded response if the monitor service is unavailable."
    )]
    async fn get_system_metrics(&self) -> String {
        health::run_get_system_metrics(&self.client).await
    }

    /// Fetch and parse key Prometheus metrics from a service.
    #[tool(
        description = "Fetch raw Prometheus metrics from any agentd service and parse key operational counters/gauges. Supports: orchestrator, notify, memory, communicate, monitor, ask, wrap, hook. Defaults to orchestrator."
    )]
    async fn get_prometheus_metrics(
        &self,
        Parameters(get_prometheus_metrics_args): Parameters<GetPrometheusMetricsArgs>,
    ) -> String {
        let GetPrometheusMetricsArgs { service } = get_prometheus_metrics_args;
        health::run_get_prometheus_metrics(&self.client, service.as_deref()).await
    }

    /// Run a curated PromQL query via the monitor service.
    #[tool(
        description = "Run a curated PromQL query against the agentd Prometheus stack via the monitor service. Call without a name to list the catalog (dispatch-success-rate, dispatch-throughput, agent-restart-rate, http-error-rate, http-p95-latency, session-cost, approvals-backlog, host-saturation, ...). window is like 15m/1h/24h; range=true returns a six-hour trend summarized per series instead of an instant value. Falls back gracefully when Prometheus is down (use get_prometheus_metrics for direct scraping)."
    )]
    async fn query_metrics(
        &self,
        Parameters(query_metrics_args): Parameters<QueryMetricsArgs>,
    ) -> String {
        let QueryMetricsArgs { name, window, range } = query_metrics_args;
        metrics::run_query_metrics(
            &self.client,
            name.as_deref(),
            window.as_deref(),
            range.unwrap_or(false),
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for AgentdMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "agentd MCP server — exposes agentd agent management, \
                 messaging, memory, knowledge, notification, approval, \
                 workflow, and diagnostic services as MCP tools. \
                 Start with `diagnose_system` or `check_service_health` for a \
                 system overview. Use `diagnose_state_mismatches` to catch \
                 subtle agent stuckness. Provision resources with \
                 `create_agent` and `create_workflow`; smoke-test new \
                 workflows with `trigger_workflow`. Browse and author \
                 per-project documents with the `knowledge_*` tools.",
        )
    }
}
