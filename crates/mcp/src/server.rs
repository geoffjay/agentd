//! MCP server implementation for agentd.
//!
//! `AgentdMcp` is the central server struct. It implements `ServerHandler`
//! via rmcp's `#[tool(tool_box)]` macro, which wires the tool routing into
//! the MCP `tools/list` and `tools/call` handlers.

use crate::client::AgentdClient;
use crate::config::AgentdMcpConfig;
use crate::tools::{
    agents, approvals, communicate, diagnostic, health, lifecycle, memory, notifications,
    orchestrator_debug, remediation, workflows,
};
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
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

#[tool(tool_box)]
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
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) to diagnose")]
        agent_id: String,
    ) -> String {
        diagnostic::run_diagnose_agent(&self.client, &agent_id).await
    }

    /// Analyse a workflow's configuration, associated agent health, and
    /// dispatch history. Identifies failure patterns and success rate issues.
    #[tool(
        description = "Run a diagnostic on a workflow: verify the associated agent is running, analyze dispatch success rate over the last 20 dispatches, and identify consecutive failure patterns. Returns a markdown report with severity-tagged issues."
    )]
    async fn diagnose_workflow(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID (UUID) to diagnose")]
        workflow_id: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "Queue name (e.g. agent-tasks, workflow-dispatch)")]
        queue_name: String,
        #[tool(param)]
        #[schemars(description = "Number of pending tasks to peek (default: 10, max: 100)")]
        peek_limit: Option<u32>,
    ) -> String {
        orchestrator_debug::run_inspect_queue(&self.client, &queue_name, peek_limit).await
    }

    /// Summarise an agent's conversation history (event counts, sessions, time range).
    #[tool(
        description = "Get a summary of an agent's conversation history: total events, per-event-type counts, session count, first/last event timestamps. Cheap way to see whether an agent has actually been productive without loading full transcripts."
    )]
    async fn get_conversation_summary(
        &self,
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID)")]
        agent_id: String,
    ) -> String {
        orchestrator_debug::run_get_conversation_summary(&self.client, &agent_id).await
    }

    /// List projects (groupings of agents and workflows).
    #[tool(
        description = "List projects defined in the orchestrator. Projects group agents and workflows for organisational purposes."
    )]
    async fn list_projects(
        &self,
        #[tool(param)]
        #[schemars(description = "Maximum projects to return (default: 50, max: 200)")]
        limit: Option<u32>,
    ) -> String {
        orchestrator_debug::run_list_projects(&self.client, limit).await
    }

    /// Get a project's details including agent and workflow counts.
    #[tool(
        description = "Get a project's details including counts of associated agents and workflows."
    )]
    async fn get_project(
        &self,
        #[tool(param)]
        #[schemars(description = "The project ID (UUID)")]
        project_id: String,
    ) -> String {
        orchestrator_debug::run_get_project(&self.client, &project_id).await
    }

    // ── Agent inspection ────────────────────────────────────────────────

    /// List all agents managed by agentd.
    #[tool(
        description = "List all agents managed by agentd, optionally filtered by status. Returns a table with agent ID, name, status, and activity state."
    )]
    async fn list_agents(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Filter by status: pending | running | stopped | failed. Omit for all agents."
        )]
        status: Option<String>,
    ) -> String {
        agents::run_list_agents(&self.client, status.as_deref()).await
    }

    /// Get full details for a specific agent.
    #[tool(
        description = "Get detailed information about a specific agent including its configuration, tool policy, model, working directory, and environment variable keys (values are redacted)."
    )]
    async fn get_agent(
        &self,
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) to inspect")]
        agent_id: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "The workflow ID (UUID) to inspect")]
        workflow_id: String,
    ) -> String {
        workflows::run_get_workflow(&self.client, &workflow_id).await
    }

    /// List dispatch records for a workflow.
    #[tool(
        description = "List dispatch records for a specific workflow showing task execution history with status and timing. Supports optional status filter and limit."
    )]
    async fn list_dispatches(
        &self,
        #[tool(param)]
        #[schemars(description = "The workflow ID (UUID) to list dispatches for")]
        workflow_id: String,
        #[tool(param)]
        #[schemars(
            description = "Filter by status: pending | dispatched | completed | failed | skipped. Omit for all."
        )]
        status: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum number of records to return (default: 20, max: 200)")]
        limit: Option<u32>,
    ) -> String {
        workflows::run_list_dispatches(&self.client, &workflow_id, status.as_deref(), limit).await
    }

    /// Get all failed dispatch records across all workflows.
    #[tool(
        description = "Get all failed dispatch records across all workflows, useful for identifying systemic issues. Returns failures sorted by most recent first."
    )]
    async fn get_failed_dispatches(
        &self,
        #[tool(param)]
        #[schemars(description = "Maximum number of records to return (default: 50, max: 200)")]
        limit: Option<u32>,
    ) -> String {
        workflows::run_get_failed_dispatches(&self.client, limit).await
    }

    // ── Notification management ─────────────────────────────────────────

    /// List notifications with optional filters.
    #[tool(
        description = "List notifications with optional filters for status and priority. Sorted by priority (highest first). Use get_actionable_notifications for a focused view of items requiring attention."
    )]
    async fn list_notifications(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Filter by status: pending | viewed | responded | dismissed | expired. Omit for all."
        )]
        status: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Filter by priority: low | normal | high | urgent. Omit for all."
        )]
        priority: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum number to return (default: 20, max: 200)")]
        limit: Option<u32>,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "The notification ID (UUID)")]
        notification_id: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "Short title for the notification")]
        title: String,
        #[tool(param)]
        #[schemars(description = "Detailed message body with diagnostic context")]
        message: String,
        #[tool(param)]
        #[schemars(description = "Priority level: low | normal | high | urgent (default: normal)")]
        priority: Option<String>,
    ) -> String {
        notifications::run_create_notification(&self.client, &title, &message, priority.as_deref())
            .await
    }

    /// Dismiss a notification.
    #[tool(
        description = "Dismiss a notification, marking it as reviewed and removing it from the active backlog."
    )]
    async fn dismiss_notification(
        &self,
        #[tool(param)]
        #[schemars(description = "The notification ID (UUID) to dismiss")]
        notification_id: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) whose approvals to list")]
        agent_id: String,
    ) -> String {
        approvals::run_get_agent_approvals(&self.client, &agent_id).await
    }

    /// Approve a pending tool use request, allowing the agent to proceed.
    #[tool(
        description = "Approve a pending tool use request, allowing the agent to proceed with the tool invocation. The agent will resume execution once approved."
    )]
    async fn approve_tool_request(
        &self,
        #[tool(param)]
        #[schemars(description = "The approval request ID (UUID) to approve")]
        approval_id: String,
    ) -> String {
        approvals::run_approve_tool_request(&self.client, &approval_id).await
    }

    /// Deny a pending tool use request with an optional reason.
    #[tool(
        description = "Deny a pending tool use request, preventing the agent from using the tool. An optional reason is sent back to the agent."
    )]
    async fn deny_tool_request(
        &self,
        #[tool(param)]
        #[schemars(description = "The approval request ID (UUID) to deny")]
        approval_id: String,
        #[tool(param)]
        #[schemars(description = "Optional reason for denial, sent back to the agent")]
        reason: Option<String>,
    ) -> String {
        approvals::run_deny_tool_request(&self.client, &approval_id, reason.as_deref()).await
    }

    // ── Communicate: rooms, participants, messages ──────────────────────

    /// List rooms in the communicate service.
    #[tool(
        description = "List rooms in the communicate service. Optionally filter by room_type (direct | group | broadcast) or project_id. Returns a table with type, name, ID, topic, and creation time."
    )]
    async fn list_rooms(
        &self,
        #[tool(param)]
        #[schemars(description = "Filter by room type: direct | group | broadcast. Omit for all.")]
        room_type: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by project ID (UUID). Omit for all.")]
        project_id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum number of rooms to return (default: 50, max: 200)")]
        limit: Option<u32>,
    ) -> String {
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
    async fn get_room(
        &self,
        #[tool(param)]
        #[schemars(description = "The room ID (UUID) to fetch")]
        room_id: String,
    ) -> String {
        communicate::run_get_room(&self.client, &room_id).await
    }

    /// List recent messages in a room.
    #[tool(
        description = "List the most recent messages in a room. Returns a table with timestamp, sender, status, and content preview. Useful for catching up on conversation activity or diagnosing what an agent has been saying."
    )]
    async fn list_messages(
        &self,
        #[tool(param)]
        #[schemars(description = "The room ID (UUID) whose messages to list")]
        room_id: String,
        #[tool(param)]
        #[schemars(description = "Maximum number of messages (default: 20, max: 200)")]
        limit: Option<u32>,
    ) -> String {
        communicate::run_list_messages(&self.client, &room_id, limit).await
    }

    /// Post a message into a room as a given sender.
    #[tool(
        description = "Send a message to a room. The sender must be a participant in the room. Useful for remediation flows where the system or an admin agent needs to post a status update or coordinate with other agents."
    )]
    async fn send_room_message(
        &self,
        #[tool(param)]
        #[schemars(description = "The room ID (UUID) to post into")]
        room_id: String,
        #[tool(param)]
        #[schemars(description = "Sender identifier (agent ID or human username)")]
        sender_id: String,
        #[tool(param)]
        #[schemars(description = "Display name for the sender")]
        sender_name: String,
        #[tool(param)]
        #[schemars(description = "Sender kind: agent | human")]
        sender_kind: String,
        #[tool(param)]
        #[schemars(description = "The message content to send")]
        content: String,
    ) -> String {
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

    // ── Memory: search, list, get ───────────────────────────────────────

    /// Semantic search across stored memories.
    #[tool(
        description = "Semantic search across agent memories. Returns matching memories ranked by relevance. Useful for discovering what knowledge agents have stored, finding context for a diagnostic, or surfacing relevant prior decisions."
    )]
    async fn search_memories(
        &self,
        #[tool(param)]
        #[schemars(description = "Search query (required, non-empty)")]
        query: String,
        #[tool(param)]
        #[schemars(description = "Filter results to memories with all of these tags")]
        tags: Option<Vec<String>>,
        #[tool(param)]
        #[schemars(description = "Filter by memory type: information | question | request")]
        memory_type: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum results (default: 10, max: 100)")]
        limit: Option<u32>,
    ) -> String {
        memory::run_search_memories(&self.client, &query, tags, memory_type.as_deref(), limit).await
    }

    /// List memories with optional filters.
    #[tool(
        description = "List memories with optional filters for type, tag, creator, and visibility. Use search_memories for content-based discovery and this tool for metadata-based browsing."
    )]
    async fn list_memories(
        &self,
        #[tool(param)]
        #[schemars(description = "Filter by memory type: information | question | request")]
        memory_type: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by a tag (single tag)")]
        tag: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by creator identifier")]
        created_by: Option<String>,
        #[tool(param)]
        #[schemars(description = "Filter by visibility: public | private | shared")]
        visibility: Option<String>,
        #[tool(param)]
        #[schemars(description = "Maximum results (default: 50, max: 200)")]
        limit: Option<u32>,
    ) -> String {
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
    async fn get_memory(
        &self,
        #[tool(param)]
        #[schemars(description = "The memory ID (e.g. mem_1718000000000_a1b2c3d4)")]
        memory_id: String,
    ) -> String {
        memory::run_get_memory(&self.client, &memory_id).await
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
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) to restart")]
        agent_id: String,
    ) -> String {
        lifecycle::run_restart_agent(&self.client, &agent_id).await
    }

    /// Send a message/prompt to a running agent.
    #[tool(
        description = "Send a message or prompt to a running agent via the orchestrator. The agent will process the message in its current session context."
    )]
    async fn send_agent_message(
        &self,
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) to message")]
        agent_id: String,
        #[tool(param)]
        #[schemars(description = "The message content or prompt to send to the agent")]
        message: String,
    ) -> String {
        lifecycle::run_send_agent_message(&self.client, &agent_id, &message).await
    }

    /// Update an agent's tool policy.
    #[tool(
        description = "Update an agent's tool policy to restrict or allow tool usage. Modes: allow_all, deny_all, require_approval, allow_list (needs tools), deny_list (needs tools)."
    )]
    async fn update_agent_tool_policy(
        &self,
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID)")]
        agent_id: String,
        #[tool(param)]
        #[schemars(
            description = "Policy mode: allow_all | deny_all | require_approval | allow_list | deny_list"
        )]
        mode: String,
        #[tool(param)]
        #[schemars(
            description = "Tool name patterns for allow_list or deny_list modes (e.g. [\"Bash\", \"Write\"])"
        )]
        tools: Option<Vec<String>>,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID) to terminate")]
        agent_id: String,
    ) -> String {
        lifecycle::run_terminate_agent(&self.client, &agent_id).await
    }

    /// Change the model an agent is using.
    #[tool(
        description = "Change the AI model an agent is using (e.g. switch from claude-sonnet to claude-opus). Takes effect for subsequent turns in the current session."
    )]
    async fn update_agent_model(
        &self,
        #[tool(param)]
        #[schemars(description = "The agent ID (UUID)")]
        agent_id: String,
        #[tool(param)]
        #[schemars(
            description = "The new model identifier (e.g. claude-opus-4-5, claude-sonnet-4-5)"
        )]
        model: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(description = "The workflow ID (UUID) to retry dispatches for")]
        workflow_id: String,
        #[tool(param)]
        #[schemars(
            description = "Only retry dispatches that failed within this many hours (default: 24)"
        )]
        hours: Option<u32>,
    ) -> String {
        remediation::run_retry_failed_dispatches(&self.client, &workflow_id, hours).await
    }

    /// Identify dispatches stuck in 'dispatched' state beyond the staleness threshold.
    #[tool(
        description = "Identify dispatch records stuck in 'dispatched' state longer than the staleness threshold. Reports stale dispatches for visibility — use restart_agent on the associated agent to unblock. Note: the orchestrator API does not support direct dispatch status updates."
    )]
    async fn cleanup_stale_dispatches(
        &self,
        #[tool(param)]
        #[schemars(description = "Consider dispatches stale after this many hours (default: 2)")]
        stale_hours: Option<u32>,
    ) -> String {
        remediation::run_cleanup_stale_dispatches(&self.client, stale_hours).await
    }

    /// Auto-approve pending tool requests matching the safe list.
    #[tool(
        description = "Automatically approve pending tool requests that match a conservative safe list of read-only tools (Read, Glob, Grep, ListFiles, WebFetch, etc.). Non-matching requests are skipped and reported. Additional tools can be added to the safe list via the parameter."
    )]
    async fn auto_approve_safe_tools(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Additional tool names to consider safe beyond the default read-only set (e.g. [\"LSP\", \"TaskOutput\"])"
        )]
        additional_safe_tools: Option<Vec<String>>,
    ) -> String {
        remediation::run_auto_approve_safe_tools(&self.client, additional_safe_tools).await
    }

    /// Bulk-dismiss old notifications that are no longer actionable.
    #[tool(
        description = "Analyze and bulk-dismiss pending notifications that are no longer actionable: expired ephemeral notifications and low-priority notifications older than the threshold. Returns an audit report of dismissed vs retained items."
    )]
    async fn resolve_notification_backlog(
        &self,
        #[tool(param)]
        #[schemars(
            description = "Dismiss low-priority notifications older than this many hours (default: 48)"
        )]
        hours: Option<u32>,
    ) -> String {
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
        #[tool(param)]
        #[schemars(
            description = "Service name: orchestrator | communicate | memory | notify | ask | wrap | monitor | hook"
        )]
        service: String,
    ) -> String {
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
        #[tool(param)]
        #[schemars(
            description = "Service: orchestrator | notify | memory | communicate | monitor | ask | wrap | hook (default: orchestrator)"
        )]
        service: Option<String>,
    ) -> String {
        health::run_get_prometheus_metrics(&self.client, service.as_deref()).await
    }
}

#[tool(tool_box)]
impl ServerHandler for AgentdMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "agentd MCP server — exposes agentd agent management, \
                 messaging, memory, notification, approval, workflow, and \
                 diagnostic services as MCP tools. \
                 Start with `diagnose_system` or `check_service_health` for a \
                 system overview. Use `diagnose_state_mismatches` to catch \
                 subtle agent stuckness."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
