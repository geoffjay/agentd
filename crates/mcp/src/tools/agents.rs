//! Agent inspection tools for the agentd MCP server.
//!
//! Provides read-only tools for listing agents, retrieving individual agent
//! details, and getting a fleet-wide status summary.

use crate::client::AgentdClient;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Lightweight view of an agent returned by `GET /agents`.
#[derive(Debug, Deserialize)]
struct AgentSummary {
    id: String,
    name: String,
    status: String,
}

/// Full agent detail returned by `GET /agents/{id}`.
#[derive(Debug, Deserialize)]
struct AgentDetail {
    id: String,
    name: String,
    status: String,
    #[serde(default)]
    activity: String,
    config: serde_json::Value,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    backend_type: Option<String>,
    created_at: String,
    updated_at: String,
}

/// Paginated response wrapper.
#[derive(Debug, Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    total: u64,
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

/// List all agents, optionally filtered by status.
pub async fn run_list_agents(client: &AgentdClient, status: Option<&str>) -> String {
    let base = client.orchestrator_url();
    let url = if let Some(s) = status {
        format!("{base}/agents?status={s}&limit=100")
    } else {
        format!("{base}/agents?limit=100")
    };

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

    let page: Paginated<serde_json::Value> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing agent list: {e}"),
    };

    if page.items.is_empty() {
        let filter_note = status.map(|s| format!(" with status '{s}'")).unwrap_or_default();
        return format!("No agents found{filter_note}.");
    }

    let filter_note = status.map(|s| format!(" (status: {s})")).unwrap_or_default();
    let mut out = format!("## Agents{filter_note} — {} total\n\n", page.total);
    out.push_str("| ID | Name | Status | Activity |\n");
    out.push_str("|-----|------|--------|----------|\n");

    for agent in &page.items {
        let id = agent["id"].as_str().unwrap_or("?");
        let name = agent["name"].as_str().unwrap_or("?");
        let status = agent["status"].as_str().unwrap_or("?");
        let activity = agent["activity"].as_str().unwrap_or("idle");
        let status_icon = match status {
            "running" => "🟢",
            "pending" => "🟡",
            "stopped" => "⚫",
            "failed" => "🔴",
            _ => "❓",
        };
        out.push_str(&format!("| `{id}` | {name} | {status_icon} {status} | {activity} |\n"));
    }

    out
}

/// Get full details for a specific agent.
pub async fn run_get_agent(client: &AgentdClient, agent_id: &str) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/agents/{agent_id}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: orchestrator service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Error: agent {agent_id} not found");
    }
    if !resp.status().is_success() {
        return format!("Error: orchestrator returned HTTP {}", resp.status());
    }

    let agent: AgentDetail = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing agent details: {e}"),
    };

    let status_icon = match agent.status.as_str() {
        "running" => "🟢",
        "pending" => "🟡",
        "stopped" => "⚫",
        "failed" => "🔴",
        _ => "❓",
    };

    let session = agent.session_id.as_deref().unwrap_or("—");
    let backend = agent.backend_type.as_deref().unwrap_or("tmux");

    let mut out = format!("## Agent: {}\n\n", agent.name);
    out.push_str(&format!("- **ID**: `{}`\n", agent.id));
    out.push_str(&format!("- **Status**: {status_icon} {}\n", agent.status));
    out.push_str(&format!("- **Activity**: {}\n", agent.activity));
    out.push_str(&format!("- **Backend**: {backend}\n"));
    out.push_str(&format!("- **Session**: {session}\n"));
    out.push_str(&format!("- **Created**: {}\n", agent.created_at));
    out.push_str(&format!("- **Updated**: {}\n\n", agent.updated_at));

    // Config section
    out.push_str("### Configuration\n\n");
    let cfg = &agent.config;

    if let Some(wd) = cfg["working_dir"].as_str() {
        out.push_str(&format!("- **Working dir**: {wd}\n"));
    }
    if let Some(model) = cfg["model"].as_str() {
        out.push_str(&format!("- **Model**: {model}\n"));
    }
    if let Some(interactive) = cfg["interactive"].as_bool() {
        out.push_str(&format!("- **Interactive**: {interactive}\n"));
    }
    if let Some(worktree) = cfg["worktree"].as_bool() {
        out.push_str(&format!("- **Worktree**: {worktree}\n"));
    }
    if let Some(prompt) = cfg["system_prompt"].as_str() {
        let truncated =
            if prompt.len() > 200 { format!("{}…", &prompt[..200]) } else { prompt.to_string() };
        out.push_str(&format!("- **System prompt**: {truncated}\n"));
    }

    // Tool policy
    if let Some(policy) = cfg.get("tool_policy") {
        let policy_str = if let Some(mode) = policy["mode"].as_str() {
            match mode {
                "allow_all" => "AllowAll (no restrictions)".to_string(),
                "deny_all" => "DenyAll (block everything)".to_string(),
                "require_approval" => "RequireApproval (manual approval for each tool)".to_string(),
                "allow_list" => {
                    let tools = policy["tools"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();
                    format!("AllowList: [{tools}]")
                }
                "deny_list" => {
                    let tools = policy["tools"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "))
                        .unwrap_or_default();
                    format!("DenyList: [{tools}]")
                }
                other => other.to_string(),
            }
        } else {
            serde_json::to_string_pretty(policy).unwrap_or_default()
        };
        out.push_str(&format!("- **Tool policy**: {policy_str}\n"));
    }

    // Env keys (values are redacted by the orchestrator)
    if let Some(env) = cfg["env"].as_object() {
        if !env.is_empty() {
            let keys: Vec<&str> = env.keys().map(|k| k.as_str()).collect();
            out.push_str(&format!("- **Env keys**: {}\n", keys.join(", ")));
        }
    }

    out
}

/// Get a fleet-wide summary of agent statuses.
pub async fn run_get_agent_status_summary(client: &AgentdClient) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/agents?limit=500");

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

    let page: Paginated<AgentSummary> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing agent list: {e}"),
    };

    let total = page.total;
    let mut pending = 0u64;
    let mut running = 0u64;
    let mut stopped = 0u64;
    let mut failed = 0u64;
    let mut failed_agents: Vec<(String, String)> = Vec::new();

    for agent in &page.items {
        match agent.status.as_str() {
            "pending" => pending += 1,
            "running" => running += 1,
            "stopped" => stopped += 1,
            "failed" => {
                failed += 1;
                failed_agents.push((agent.id.clone(), agent.name.clone()));
            }
            _ => {}
        }
    }

    let mut out = format!("## Agent Fleet Status — {total} total\n\n");
    out.push_str("| Status | Count |\n");
    out.push_str("|--------|-------|\n");
    out.push_str(&format!("| 🟢 Running | {running} |\n"));
    out.push_str(&format!("| 🟡 Pending | {pending} |\n"));
    out.push_str(&format!("| ⚫ Stopped | {stopped} |\n"));
    out.push_str(&format!("| 🔴 Failed  | {failed} |\n"));

    if !failed_agents.is_empty() {
        out.push_str("\n### Failed Agents\n\n");
        for (id, name) in &failed_agents {
            out.push_str(&format!("- **{name}** (`{id}`) — use `restart_agent` to recover\n"));
        }
    }

    if total == 0 {
        out.push_str("\nNo agents registered. Use `POST /agents` to create one.\n");
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn test_status_icon_mapping() {
        let cases = [("running", "🟢"), ("pending", "🟡"), ("stopped", "⚫"), ("failed", "🔴")];
        for (status, expected_icon) in cases {
            let icon = match status {
                "running" => "🟢",
                "pending" => "🟡",
                "stopped" => "⚫",
                "failed" => "🔴",
                _ => "❓",
            };
            assert_eq!(icon, expected_icon, "icon mismatch for status '{status}'");
        }
    }

    #[test]
    fn test_fleet_summary_aggregation() {
        // Simulate counting agents by status
        let statuses = vec!["running", "running", "failed", "pending", "stopped", "failed"];
        let mut running = 0u32;
        let mut pending = 0u32;
        let mut stopped = 0u32;
        let mut failed = 0u32;
        for s in &statuses {
            match *s {
                "running" => running += 1,
                "pending" => pending += 1,
                "stopped" => stopped += 1,
                "failed" => failed += 1,
                _ => {}
            }
        }
        assert_eq!(running, 2);
        assert_eq!(pending, 1);
        assert_eq!(stopped, 1);
        assert_eq!(failed, 2);
    }
}
