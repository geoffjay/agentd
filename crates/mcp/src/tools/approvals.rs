//! Approval management tool implementations.
//!
//! Thin wrappers over the orchestrator `/approvals` endpoints, providing
//! visibility into — and control over — the pending tool approval queue.

use crate::client::AgentdClient;
use anyhow::Result;
use serde_json::json;
use std::fmt::Write;

// ── Formatting helpers ─────────────────────────────────────────────────────

/// Render a single approval record as a readable markdown block.
fn format_approval(a: &serde_json::Value) -> String {
    let id = a["id"].as_str().unwrap_or("unknown");
    let agent_id = a["agent_id"].as_str().unwrap_or("unknown");
    let tool_name = a["tool_name"].as_str().unwrap_or("unknown");
    let status = a["status"].as_str().unwrap_or("unknown");
    let created_at = a["created_at"].as_str().unwrap_or("unknown");
    let expires_at = a["expires_at"].as_str().unwrap_or("never");

    // Pretty-print tool_input, truncated to 300 chars to avoid blobs
    let input_display = match serde_json::to_string_pretty(&a["tool_input"]) {
        Ok(pretty) if pretty.len() > 300 => format!("{}…", &pretty[..300]),
        Ok(pretty) => pretty,
        Err(_) => a["tool_input"].to_string(),
    };

    let expired_note = if status == "TimedOut" { " *(expired)*" } else { "" };

    format!(
        "- **ID:** `{id}`{expired_note}\n  \
         **Agent:** `{agent_id}`\n  \
         **Tool:** `{tool_name}`\n  \
         **Status:** {status}\n  \
         **Requested:** {created_at}  **Expires:** {expires_at}\n  \
         **Input:**\n  ```json\n  {input_display}\n  ```"
    )
}

/// Render a paginated list of approvals.
fn format_approval_list(resp: &serde_json::Value, title: &str) -> String {
    let mut out = String::new();
    let total = resp["total"].as_u64().unwrap_or(0);

    writeln!(out, "# {title}").ok();

    if total == 0 {
        writeln!(out, "\nNo approvals found.").ok();
        return out;
    }

    writeln!(out, "\n**Total:** {total}\n").ok();

    let empty = vec![];
    let items = resp["items"].as_array().unwrap_or(&empty);
    for item in items {
        writeln!(out, "{}", format_approval(item)).ok();
        writeln!(out).ok();
    }

    if total > items.len() as u64 {
        writeln!(out, "*Showing first {} of {total} results.*", items.len()).ok();
    }

    out
}

// ── Tool implementations ───────────────────────────────────────────────────

/// List all pending (and recently expired) tool approval requests.
pub async fn run_list_pending_approvals(client: &AgentdClient) -> String {
    let url = format!("{}/approvals?status=Pending&limit=50", client.orchestrator_url());
    match client.get::<serde_json::Value>(&url).await {
        Ok(resp) => format_approval_list(&resp, "Pending Tool Approval Requests"),
        Err(e) => format!(
            "# Pending Tool Approvals\n\n🔴 Error fetching approvals: {e}\n\
             → Verify the orchestrator is running: `check_service_health`"
        ),
    }
}

/// List pending tool approval requests for a specific agent.
pub async fn run_get_agent_approvals(client: &AgentdClient, agent_id: &str) -> String {
    let url = format!("{}/agents/{agent_id}/approvals?limit=50", client.orchestrator_url());
    match client.get::<serde_json::Value>(&url).await {
        Ok(resp) => {
            format_approval_list(&resp, &format!("Tool Approval Requests for Agent `{agent_id}`"))
        }
        Err(e) => format!("# Agent Approvals\n\n🔴 Error fetching approvals for `{agent_id}`: {e}"),
    }
}

/// Approve a pending tool use request.
pub async fn run_approve_tool_request(client: &AgentdClient, approval_id: &str) -> String {
    let url = format!("{}/approvals/{approval_id}/approve", client.orchestrator_url());
    // ApprovalActionRequest — reason is optional, not needed for approvals
    match post_action(client, &url, None).await {
        Ok(_) => format!(
            "✅ Approval `{approval_id}` granted. The agent may proceed with the tool invocation."
        ),
        Err(e) => format!("🔴 Failed to approve `{approval_id}`: {e}"),
    }
}

/// Deny a pending tool use request with an optional reason.
pub async fn run_deny_tool_request(
    client: &AgentdClient,
    approval_id: &str,
    reason: Option<&str>,
) -> String {
    let url = format!("{}/approvals/{approval_id}/deny", client.orchestrator_url());
    match post_action(client, &url, reason).await {
        Ok(_) => {
            let reason_note = reason.map(|r| format!(" Reason: *{r}*")).unwrap_or_default();
            format!("✅ Approval `{approval_id}` denied.{reason_note} The agent's tool request has been rejected.")
        }
        Err(e) => format!("🔴 Failed to deny `{approval_id}`: {e}"),
    }
}

/// POST an approve/deny action with an optional reason body.
async fn post_action(
    client: &AgentdClient,
    url: &str,
    reason: Option<&str>,
) -> Result<serde_json::Value> {
    let body = json!({ "reason": reason });
    client.post::<serde_json::Value, serde_json::Value>(url, &body).await
}
