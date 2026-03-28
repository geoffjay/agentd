//! Workflow and dispatch inspection tools for the agentd MCP server.
//!
//! Provides read-only tools for listing workflows, getting workflow details,
//! and inspecting dispatch history and failure patterns.

use crate::client::AgentdClient;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Paginated response wrapper.
#[derive(Debug, Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    total: u64,
}

/// Workflow summary (used in list and detail views).
#[derive(Debug, Deserialize)]
struct WorkflowItem {
    id: String,
    name: String,
    agent_id: String,
    #[serde(alias = "source_config")]
    trigger_config: serde_json::Value,
    prompt_template: String,
    poll_interval_secs: u64,
    enabled: bool,
    tool_policy: serde_json::Value,
    created_at: String,
    updated_at: String,
}

/// Dispatch record.
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn trigger_type(cfg: &serde_json::Value) -> &str {
    cfg["type"].as_str().unwrap_or("unknown")
}

fn enabled_icon(enabled: bool) -> &'static str {
    if enabled {
        "✅"
    } else {
        "⏸️"
    }
}

fn dispatch_icon(status: &str) -> &'static str {
    match status {
        "completed" => "✅",
        "failed" => "🔴",
        "dispatched" => "🔄",
        "pending" => "🟡",
        "skipped" => "⏭️",
        _ => "❓",
    }
}

fn truncate(s: &str, max: usize) -> String {
    // Truncate at a char boundary
    if s.len() <= max {
        s.to_string()
    } else {
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i < max)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max);
        format!("{}…", &s[..boundary])
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

/// List all configured workflows.
pub async fn run_list_workflows(client: &AgentdClient) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/workflows?limit=100");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {}", resp.status());
    }

    let page: Paginated<WorkflowItem> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing workflow list: {e}"),
    };

    if page.items.is_empty() {
        return "No workflows configured. Use `POST /workflows` to create one.".to_string();
    }

    let mut out = format!("## Workflows — {} total\n\n", page.total);
    out.push_str("| ID | Name | Trigger | Interval | Enabled |\n");
    out.push_str("|----|------|---------|----------|---------|\n");

    for wf in &page.items {
        let trigger = trigger_type(&wf.trigger_config);
        let interval = format!("{}s", wf.poll_interval_secs);
        let enabled = enabled_icon(wf.enabled);
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            wf.id, wf.name, trigger, interval, enabled
        ));
    }

    out
}

/// Get full configuration of a workflow.
pub async fn run_get_workflow(client: &AgentdClient, workflow_id: &str) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/workflows/{workflow_id}");

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

    let wf: WorkflowItem = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing workflow details: {e}"),
    };

    let trigger = trigger_type(&wf.trigger_config);
    let enabled = enabled_icon(wf.enabled);

    let mut out = format!("## Workflow: {}\n\n", wf.name);
    out.push_str(&format!("- **ID**: `{}`\n", wf.id));
    out.push_str(&format!("- **Agent ID**: `{}`\n", wf.agent_id));
    out.push_str(&format!("- **Trigger type**: {trigger}\n"));
    out.push_str(&format!("- **Poll interval**: {}s\n", wf.poll_interval_secs));
    out.push_str(&format!("- **Enabled**: {enabled}\n"));
    out.push_str(&format!("- **Created**: {}\n", wf.created_at));
    out.push_str(&format!("- **Updated**: {}\n\n", wf.updated_at));

    // Trigger config details
    out.push_str("### Trigger Configuration\n\n");
    out.push_str("```json\n");
    out.push_str(&serde_json::to_string_pretty(&wf.trigger_config).unwrap_or_default());
    out.push_str("\n```\n\n");

    // Tool policy
    let policy_mode = wf.tool_policy["mode"].as_str().unwrap_or("allow_all");
    out.push_str(&format!("### Tool Policy\n\n- **Mode**: {policy_mode}\n"));
    if let Some(tools) = wf.tool_policy["tools"].as_array() {
        let names: Vec<_> = tools.iter().filter_map(|v| v.as_str()).collect();
        if !names.is_empty() {
            out.push_str(&format!("- **Tools**: {}\n", names.join(", ")));
        }
    }

    // Prompt template (truncated)
    out.push_str("\n### Prompt Template\n\n```\n");
    out.push_str(&truncate(&wf.prompt_template, 500));
    out.push_str("\n```\n");

    out
}

/// List dispatch records for a workflow.
pub async fn run_list_dispatches(
    client: &AgentdClient,
    workflow_id: &str,
    status: Option<&str>,
    limit: Option<u32>,
) -> String {
    let base = client.orchestrator_url();
    let fetch_limit = limit.unwrap_or(20).clamp(1, 200);
    // Fetch more than requested so we can filter by status client-side
    let fetch_count = if status.is_some() { fetch_limit * 5 } else { fetch_limit };
    let url = format!("{base}/workflows/{workflow_id}/history?limit={fetch_count}");

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

    // Apply status filter client-side
    let items: Vec<&DispatchItem> = page
        .items
        .iter()
        .filter(|d| status.is_none_or(|s| d.status == s))
        .take(fetch_limit as usize)
        .collect();

    if items.is_empty() {
        let filter = status.map(|s| format!(" with status '{s}'")).unwrap_or_default();
        return format!("No dispatch records found for workflow {workflow_id}{filter}.");
    }

    let status_note = status.map(|s| format!(" (status: {s})")).unwrap_or_default();
    let mut out =
        format!("## Dispatch History: {workflow_id}{status_note} — {} records\n\n", items.len());
    out.push_str("| Status | Source ID | Dispatched | Completed | Prompt |\n");
    out.push_str("|--------|-----------|------------|-----------|--------|\n");

    for d in &items {
        let icon = dispatch_icon(&d.status);
        let completed = d.completed_at.as_deref().unwrap_or("—");
        let prompt = truncate(&d.prompt_sent, 60);
        out.push_str(&format!(
            "| {icon} {} | `{}` | {} | {} | {} |\n",
            d.status, d.source_id, d.dispatched_at, completed, prompt
        ));
    }

    out
}

/// Get all failed dispatches across all workflows.
pub async fn run_get_failed_dispatches(client: &AgentdClient, limit: Option<u32>) -> String {
    let base = client.orchestrator_url();
    let max_results = limit.unwrap_or(50).clamp(1, 200) as usize;

    // First, get all workflows
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

    let wf_page: Paginated<WorkflowItem> = match wf_resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing workflow list: {e}"),
    };

    if wf_page.items.is_empty() {
        return "No workflows configured — no dispatch records to inspect.".to_string();
    }

    // For each workflow, fetch recent dispatch history and filter for failures
    let mut failed: Vec<(String, String, DispatchItem)> = Vec::new(); // (wf_name, wf_id, dispatch)

    for wf in &wf_page.items {
        let hist_url = format!("{base}/workflows/{}/history?limit=50", wf.id);
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
            if d.status == "failed" {
                failed.push((wf.name.clone(), wf.id.clone(), d));
            }
        }
    }

    // Sort by dispatched_at descending (string comparison works for ISO 8601)
    failed.sort_by(|a, b| b.2.dispatched_at.cmp(&a.2.dispatched_at));
    failed.truncate(max_results);

    if failed.is_empty() {
        return format!("✅ No failed dispatches found across {} workflow(s).", wf_page.total);
    }

    let mut out = format!("## Failed Dispatches — {} failures found\n\n", failed.len());
    out.push_str("| Workflow | Source ID | Dispatched | Prompt |\n");
    out.push_str("|----------|-----------|------------|--------|\n");

    for (wf_name, _wf_id, d) in &failed {
        let prompt = truncate(&d.prompt_sent, 60);
        out.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            wf_name, d.source_id, d.dispatched_at, prompt
        ));
    }

    out.push_str(
        "\n> Use `diagnose_workflow` on the relevant workflow for a deeper failure analysis.\n",
    );

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let s = "a".repeat(300);
        let result = truncate(&s, 200);
        assert!(result.len() <= 204); // 200 chars + ellipsis
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_dispatch_icon() {
        assert_eq!(dispatch_icon("completed"), "✅");
        assert_eq!(dispatch_icon("failed"), "🔴");
        assert_eq!(dispatch_icon("dispatched"), "🔄");
        assert_eq!(dispatch_icon("pending"), "🟡");
        assert_eq!(dispatch_icon("skipped"), "⏭️");
        assert_eq!(dispatch_icon("unknown"), "❓");
    }

    #[test]
    fn test_enabled_icon() {
        assert_eq!(enabled_icon(true), "✅");
        assert_eq!(enabled_icon(false), "⏸️");
    }
}
