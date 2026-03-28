//! MCP server implementation for agentd.
//!
//! `AgentdMcp` is the central server struct. It implements `ServerHandler`
//! via rmcp's `#[tool(tool_box)]` macro, which wires the tool routing into
//! the MCP `tools/list` and `tools/call` handlers.

use crate::client::AgentdClient;
use crate::config::AgentdMcpConfig;
use crate::tools::{approvals, diagnostic};
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

    /// Test connectivity to every agentd service by probing its /health endpoint.
    #[tool(
        description = "Test connectivity between the MCP server and all agentd services (orchestrator, communicate, memory, notify, ask, wrap, monitor). Returns a table showing which services are reachable and which are not."
    )]
    async fn check_connectivity(&self) -> String {
        diagnostic::run_check_connectivity(&self.client).await
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
}

#[tool(tool_box)]
impl ServerHandler for AgentdMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "agentd MCP server — exposes agentd agent management, \
                 messaging, memory, notification, approval, workflow, and \
                 diagnostic services as MCP tools. \
                 Start with `diagnose_system` or `check_connectivity` for a \
                 system overview."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
