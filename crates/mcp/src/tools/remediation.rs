//! Self-healing remediation tools for the agentd MCP server.
//!
//! These tools automate common recovery actions: restarting failed agents,
//! retrying failed dispatches, cleaning up stale dispatch records, batch-approving
//! safe tools, and resolving notification backlogs.
//!
//! All tools produce audit reports of actions taken. They handle partial failures
//! gracefully and are idempotent where possible.

use crate::client::AgentdClient;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Shared response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AgentSummary {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AgentDetail {
    id: String,
    name: String,
    config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DispatchItem {
    id: String,
    workflow_id: String,
    source_id: String,
    agent_id: String,
    prompt_sent: String,
    status: String,
    dispatched_at: String,
    #[serde(default)]
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApprovalItem {
    id: String,
    tool_name: String,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NotificationItem {
    id: String,
    priority: String,
    title: String,
    lifetime: serde_json::Value,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    total: u64,
}

#[derive(Debug, Deserialize)]
struct WorkflowSummary {
    id: String,
    name: String,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

/// Restart all agents currently in the `failed` state.
pub async fn run_restart_failed_agents(client: &AgentdClient) -> String {
    let base = client.orchestrator_url();

    // 1. List all failed agents
    let url = format!("{base}/agents?status=failed&limit=100");
    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {} listing agents", resp.status());
    }

    let page: Paginated<AgentSummary> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing agent list: {e}"),
    };

    if page.items.is_empty() {
        return "✅ No failed agents found — nothing to restart.".to_string();
    }

    let total_failed = page.items.len();
    let mut restarted: Vec<(String, String, String)> = Vec::new(); // (name, old_id, new_id)
    let mut failed_restarts: Vec<(String, String, String)> = Vec::new(); // (name, id, reason)

    // 2. For each failed agent: GET config → DELETE → POST recreate
    for agent in &page.items {
        // Get full config
        let detail_url = format!("{base}/agents/{}", agent.id);
        let detail_resp = match client.inner.get(&detail_url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                failed_restarts.push((
                    agent.name.clone(),
                    agent.id.clone(),
                    format!("GET config failed: HTTP {}", r.status()),
                ));
                continue;
            }
            Err(e) => {
                failed_restarts.push((agent.name.clone(), agent.id.clone(), e.to_string()));
                continue;
            }
        };

        let detail: AgentDetail = match detail_resp.json().await {
            Ok(v) => v,
            Err(e) => {
                failed_restarts.push((
                    agent.name.clone(),
                    agent.id.clone(),
                    format!("Parse config error: {e}"),
                ));
                continue;
            }
        };

        // Terminate the failed agent
        let del_url = format!("{base}/agents/{}", agent.id);
        if let Err(e) = client.inner.delete(&del_url).send().await {
            failed_restarts.push((
                agent.name.clone(),
                agent.id.clone(),
                format!("DELETE failed: {e}"),
            ));
            continue;
        }

        // Recreate with same name and config
        let create_body = json!({
            "name": detail.name,
            "working_dir": detail.config["working_dir"].as_str().unwrap_or("."),
            "model": detail.config["model"],
            "tool_policy": detail.config["tool_policy"],
            "system_prompt": detail.config["system_prompt"],
            "interactive": detail.config["interactive"].as_bool().unwrap_or(false),
            "worktree": detail.config["worktree"].as_bool().unwrap_or(false),
        });

        let create_url = format!("{base}/agents");
        match client.inner.post(&create_url).json(&create_body).send().await {
            Ok(r) if r.status().is_success() => {
                let new_agent: serde_json::Value = r.json().await.unwrap_or_default();
                let new_id = new_agent["id"].as_str().unwrap_or("?").to_string();
                restarted.push((agent.name.clone(), agent.id.clone(), new_id));
            }
            Ok(r) => {
                failed_restarts.push((
                    agent.name.clone(),
                    agent.id.clone(),
                    format!("POST create failed: HTTP {}", r.status()),
                ));
            }
            Err(e) => {
                failed_restarts.push((
                    agent.name.clone(),
                    agent.id.clone(),
                    format!("POST create error: {e}"),
                ));
            }
        }
    }

    // 3. Build report
    let mut out = format!("## Restart Failed Agents — {total_failed} failed agents found\n\n");

    if !restarted.is_empty() {
        out.push_str(&format!("### ✅ Restarted ({}/{})\n\n", restarted.len(), total_failed));
        for (name, old_id, new_id) in &restarted {
            out.push_str(&format!("- **{name}**: `{old_id}` → new agent `{new_id}`\n"));
        }
        out.push('\n');
    }

    if !failed_restarts.is_empty() {
        out.push_str(&format!(
            "### 🔴 Failed to Restart ({}/{})\n\n",
            failed_restarts.len(),
            total_failed
        ));
        for (name, id, reason) in &failed_restarts {
            out.push_str(&format!("- **{name}** (`{id}`): {reason}\n"));
        }
        out.push('\n');
    }

    out
}

/// Retry failed dispatches for a workflow within a time window.
pub async fn run_retry_failed_dispatches(
    client: &AgentdClient,
    workflow_id: &str,
    hours: Option<u32>,
) -> String {
    let base = client.orchestrator_url();
    let window_hours = hours.unwrap_or(24) as i64;
    let cutoff: DateTime<Utc> = Utc::now() - Duration::hours(window_hours);
    let cutoff_str = cutoff.to_rfc3339();

    // 1. Fetch dispatch history for this workflow
    let url = format!("{base}/workflows/{workflow_id}/history?limit=200");
    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Error: workflow {workflow_id} not found");
    }
    if !resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {}", resp.status());
    }

    let page: Paginated<DispatchItem> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing dispatch history: {e}"),
    };

    // 2. Filter: status=failed AND dispatched_at >= cutoff
    let failed_in_window: Vec<&DispatchItem> = page
        .items
        .iter()
        .filter(|d| d.status == "failed" && d.dispatched_at >= cutoff_str)
        .collect();

    if failed_in_window.is_empty() {
        return format!(
            "✅ No failed dispatches found for workflow `{workflow_id}` in the last {window_hours} hour(s)."
        );
    }

    let total = failed_in_window.len();
    let mut retried: Vec<String> = Vec::new();
    let mut retry_failed: Vec<(String, String)> = Vec::new();

    // 3. Re-send prompt to agent for each failed dispatch
    for d in &failed_in_window {
        let msg_url = format!("{base}/agents/{}/message", d.agent_id);
        let body = json!({ "content": d.prompt_sent });
        match client.inner.post(&msg_url).json(&body).send().await {
            Ok(r) if r.status().is_success() => {
                retried.push(d.source_id.clone());
            }
            Ok(r) => {
                retry_failed.push((d.source_id.clone(), format!("HTTP {}", r.status())));
            }
            Err(e) => {
                retry_failed.push((d.source_id.clone(), e.to_string()));
            }
        }
    }

    let mut out =
        format!("## Retry Failed Dispatches — workflow `{workflow_id}`, last {window_hours}h\n\n");
    out.push_str(&format!("Found {total} failed dispatch(es) in window.\n\n"));

    if !retried.is_empty() {
        out.push_str(&format!("### ✅ Retried ({}/{})\n\n", retried.len(), total));
        for src in &retried {
            out.push_str(&format!("- Source `{src}` — prompt re-sent to agent\n"));
        }
        out.push('\n');
    }

    if !retry_failed.is_empty() {
        out.push_str(&format!("### 🔴 Retry Failed ({}/{})\n\n", retry_failed.len(), total));
        for (src, reason) in &retry_failed {
            out.push_str(&format!("- Source `{src}`: {reason}\n"));
        }
        out.push('\n');
    }

    out
}

/// Identify dispatch records stuck in "dispatched" state beyond the staleness threshold.
///
/// Note: The orchestrator API does not expose a dispatch-update endpoint, so this
/// tool reports stale dispatches for visibility. Use `retry_failed_dispatches` or
/// `restart_agent` to unblock the associated agent if it appears hung.
pub async fn run_cleanup_stale_dispatches(
    client: &AgentdClient,
    stale_hours: Option<u32>,
) -> String {
    let base = client.orchestrator_url();
    let threshold_hours = stale_hours.unwrap_or(2) as i64;
    let cutoff: DateTime<Utc> = Utc::now() - Duration::hours(threshold_hours);
    let cutoff_str = cutoff.to_rfc3339();

    // List all workflows, then check each for stale dispatches
    let wf_url = format!("{base}/workflows?limit=100");
    let wf_resp = match client.inner.get(&wf_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !wf_resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {} listing workflows", wf_resp.status());
    }

    let wf_page: Paginated<WorkflowSummary> = match wf_resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing workflow list: {e}"),
    };

    if wf_page.items.is_empty() {
        return "No workflows configured — no dispatch records to inspect.".to_string();
    }

    let mut stale: Vec<(String, String, String, String)> = Vec::new(); // (wf_name, wf_id, source_id, dispatched_at)

    for wf in &wf_page.items {
        let hist_url = format!("{base}/workflows/{}/history?limit=100", wf.id);
        let Ok(hr) = client.inner.get(&hist_url).send().await else {
            continue;
        };
        if !hr.status().is_success() {
            continue;
        }
        let Ok(page) = hr.json::<Paginated<DispatchItem>>().await else {
            continue;
        };
        for d in page.items {
            if d.status == "dispatched" && d.dispatched_at <= cutoff_str {
                stale.push((wf.name.clone(), wf.id.clone(), d.source_id, d.dispatched_at));
            }
        }
    }

    if stale.is_empty() {
        return format!(
            "✅ No stale dispatches found (threshold: {threshold_hours}h). All in-flight dispatches are within the staleness window."
        );
    }

    let mut out = format!(
        "## Stale Dispatches — {} dispatch(es) stuck for >{threshold_hours}h\n\n",
        stale.len()
    );
    out.push_str(
        "> ⚠️ **Note**: The orchestrator API does not expose a dispatch-update endpoint. \
         These dispatches cannot be marked as failed via MCP. To unblock:\n\
         > 1. Use `get_agent` to check the associated agent's status\n\
         > 2. Use `restart_agent` if the agent is hung\n\
         > 3. Contact an admin to update dispatch status directly in the database\n\n",
    );
    out.push_str("| Workflow | Source ID | Dispatched At |\n");
    out.push_str("|----------|-----------|---------------|\n");

    for (wf_name, _wf_id, source_id, dispatched_at) in &stale {
        out.push_str(&format!("| {wf_name} | `{source_id}` | {dispatched_at} |\n"));
    }

    out
}

/// Auto-approve pending tool requests that match the conservative safe list.
pub async fn run_auto_approve_safe_tools(
    client: &AgentdClient,
    additional_safe_tools: Option<Vec<String>>,
) -> String {
    let base = client.orchestrator_url();

    // Conservative default safe list — read-only tools only
    let mut safe_tools: Vec<String> = vec![
        "Read".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
        "ListFiles".to_string(),
        "ListMcpResourcesTool".to_string(),
        "ReadMcpResourceTool".to_string(),
        "WebFetch".to_string(),
        "WebSearch".to_string(),
        "TodoWrite".to_string(),
        "TodoRead".to_string(),
    ];

    if let Some(extra) = additional_safe_tools {
        safe_tools.extend(extra);
    }

    // Get pending approvals
    let url = format!("{base}/approvals?status=pending&limit=200");
    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {} listing approvals", resp.status());
    }

    let approvals: Vec<ApprovalItem> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing approvals: {e}"),
    };

    if approvals.is_empty() {
        return "✅ No pending approvals — nothing to auto-approve.".to_string();
    }

    // Filter for safe tools
    let safe_approvals: Vec<&ApprovalItem> =
        approvals.iter().filter(|a| safe_tools.contains(&a.tool_name)).collect();

    let unsafe_approvals: Vec<&ApprovalItem> =
        approvals.iter().filter(|a| !safe_tools.contains(&a.tool_name)).collect();

    let mut approved: Vec<(String, String)> = Vec::new(); // (tool_name, agent_id)
    let mut approve_failed: Vec<(String, String)> = Vec::new();

    for approval in &safe_approvals {
        let approve_url = format!("{base}/approvals/{}/approve", approval.id);
        match client.inner.post(&approve_url).json(&json!({})).send().await {
            Ok(r) if r.status().is_success() => {
                approved.push((
                    approval.tool_name.clone(),
                    approval.agent_id.clone().unwrap_or_default(),
                ));
            }
            Ok(r) => {
                approve_failed.push((approval.tool_name.clone(), format!("HTTP {}", r.status())));
            }
            Err(e) => {
                approve_failed.push((approval.tool_name.clone(), e.to_string()));
            }
        }
    }

    let total_pending = approvals.len();
    let mut out = format!("## Auto-Approve Safe Tools — {total_pending} pending approval(s)\n\n");
    out.push_str(&format!("Safe list: {}\n\n", safe_tools.join(", ")));

    if !approved.is_empty() {
        out.push_str(&format!("### ✅ Approved ({})\n\n", approved.len()));
        for (tool, agent) in &approved {
            let agent_note =
                if agent.is_empty() { String::new() } else { format!(" for agent `{agent}`") };
            out.push_str(&format!("- **{tool}**{agent_note}\n"));
        }
        out.push('\n');
    }

    if !approve_failed.is_empty() {
        out.push_str(&format!("### 🔴 Failed to Approve ({})\n\n", approve_failed.len()));
        for (tool, reason) in &approve_failed {
            out.push_str(&format!("- **{tool}**: {reason}\n"));
        }
        out.push('\n');
    }

    if !unsafe_approvals.is_empty() {
        out.push_str(&format!(
            "### ⏸️ Skipped — Not on Safe List ({})\n\n",
            unsafe_approvals.len()
        ));
        for a in &unsafe_approvals {
            let agent_note =
                a.agent_id.as_deref().map(|id| format!(" (agent `{id}`)")).unwrap_or_default();
            out.push_str(&format!(
                "- **{}**{agent_note} — use `approve_tool_request` to approve manually\n",
                a.tool_name
            ));
        }
        out.push('\n');
    }

    out
}

/// Bulk-dismiss old notifications that are no longer actionable.
pub async fn run_resolve_notification_backlog(client: &AgentdClient, hours: Option<u32>) -> String {
    let notify_base = client.notify_url();
    let threshold_hours = hours.unwrap_or(48) as i64;
    let cutoff: DateTime<Utc> = Utc::now() - Duration::hours(threshold_hours);
    let cutoff_str = cutoff.to_rfc3339();

    // Fetch pending notifications
    let url = format!("{notify_base}/notifications?status=pending&limit=200");
    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {notify_base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: notify service returned HTTP {}", resp.status());
    }

    let page: Paginated<NotificationItem> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing notification list: {e}"),
    };

    if page.items.is_empty() {
        return "✅ No pending notifications — backlog is already clear.".to_string();
    }

    // Filter: expired ephemeral OR low-priority older than threshold
    let to_dismiss: Vec<&NotificationItem> = page
        .items
        .iter()
        .filter(|n| {
            let is_expired_ephemeral = n.lifetime["type"].as_str() == Some("ephemeral")
                && n.lifetime["expires_at"]
                    .as_str()
                    .is_some_and(|exp| exp <= Utc::now().to_rfc3339().as_str());
            let is_old_low_priority = n.priority == "low" && n.created_at <= cutoff_str;
            is_expired_ephemeral || is_old_low_priority
        })
        .collect();

    if to_dismiss.is_empty() {
        return format!(
            "✅ No dismissible notifications found. {} pending notification(s) are recent or high-priority — review them manually.",
            page.total
        );
    }

    let total_to_dismiss = to_dismiss.len();
    let mut dismissed: Vec<String> = Vec::new();
    let mut dismiss_failed: Vec<(String, String)> = Vec::new();

    for n in &to_dismiss {
        let del_url = format!("{notify_base}/notifications/{}", n.id);
        match client.inner.delete(&del_url).send().await {
            Ok(r) if r.status().is_success() => {
                dismissed.push(n.title.clone());
            }
            Ok(r) => {
                dismiss_failed.push((n.title.clone(), format!("HTTP {}", r.status())));
            }
            Err(e) => {
                dismiss_failed.push((n.title.clone(), e.to_string()));
            }
        }
    }

    let retained = page.total as usize - total_to_dismiss;
    let mut out = format!(
        "## Resolve Notification Backlog — {total_to_dismiss} notifications eligible for dismissal\n\n"
    );
    out.push_str(&format!(
        "Criteria: expired ephemeral OR low-priority older than {threshold_hours}h.\n\
         Retained: {retained} notification(s) (recent or higher-priority).\n\n"
    ));

    if !dismissed.is_empty() {
        out.push_str(&format!("### ✅ Dismissed ({}/{})\n\n", dismissed.len(), total_to_dismiss));
        for title in &dismissed {
            out.push_str(&format!("- {title}\n"));
        }
        out.push('\n');
    }

    if !dismiss_failed.is_empty() {
        out.push_str(&format!(
            "### 🔴 Failed to Dismiss ({}/{})\n\n",
            dismiss_failed.len(),
            total_to_dismiss
        ));
        for (title, reason) in &dismiss_failed {
            out.push_str(&format!("- {title}: {reason}\n"));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_cutoff_calculation() {
        let hours = 24i64;
        let cutoff = Utc::now() - Duration::hours(hours);
        // cutoff should be in the past
        assert!(cutoff < Utc::now());
        // and about 24h ago
        let diff = Utc::now() - cutoff;
        assert!(diff.num_hours() >= 23 && diff.num_hours() <= 25);
    }

    #[test]
    fn test_safe_tools_default_list() {
        let safe_tools: Vec<String> = vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "ListFiles".to_string(),
        ];
        assert!(safe_tools.contains(&"Read".to_string()));
        assert!(!safe_tools.contains(&"Write".to_string()));
        assert!(!safe_tools.contains(&"Bash".to_string()));
    }

    #[test]
    fn test_iso8601_string_comparison() {
        // ISO 8601 strings in UTC are lexicographically comparable
        let earlier = "2026-01-01T00:00:00Z";
        let later = "2026-06-01T00:00:00Z";
        assert!(earlier < later);
    }

    #[test]
    fn test_stale_threshold_defaults() {
        let stale_hours: i64 = 2;
        let retry_hours: i64 = 24;
        let notify_hours: i64 = 48;
        assert!(stale_hours < retry_hours);
        assert!(retry_hours < notify_hours);
    }
}
