//! Deep orchestrator diagnostics: state-mismatch detection, queue inspection,
//! conversation summaries, and project navigation.
//!
//! These tools surface orchestrator internals that the original MCP surface
//! left invisible: orphan WebSocket connections, queue backpressure, what an
//! agent has actually been doing, and project groupings.

use crate::client::AgentdClient;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DebugAgent {
    id: String,
    name: String,
    status: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DebugSummary {
    total_agents: u64,
    running: u64,
    ws_connected: u64,
    #[serde(default)]
    running_but_disconnected: Vec<String>,
    #[serde(default)]
    connected_but_not_running: Vec<String>,
    active_workflows: u64,
}

#[derive(Debug, Deserialize)]
struct DebugAgentsResponse {
    agents: Vec<DebugAgent>,
    #[serde(default)]
    orphan_connections: Vec<String>,
    summary: DebugSummary,
}

#[derive(Debug, Deserialize)]
struct QueueStats {
    pending: u64,
    processing: u64,
    completed: u64,
    failed: u64,
    #[serde(default)]
    dead: u64,
}

#[derive(Debug, Deserialize)]
struct QueueTask {
    title: String,
    priority: i32,
    status: String,
    retry_count: i32,
    max_retries: i32,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ConversationSummary {
    agent_id: String,
    total_events: u64,
    event_counts: serde_json::Map<String, serde_json::Value>,
    session_count: u64,
    #[serde(default)]
    first_event_at: Option<String>,
    #[serde(default)]
    last_event_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Project {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ProjectDetail {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    created_at: String,
    updated_at: String,
    agent_count: u64,
    workflow_count: u64,
}

#[derive(Debug, Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    total: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', " ")
    } else {
        let mut t = s[..max].replace('\n', " ");
        t.push('…');
        t
    }
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

pub async fn run_diagnose_state_mismatches(client: &AgentdClient) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/debug/agents");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: orchestrator unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        return format!("Error: HTTP {} fetching debug state", resp.status());
    }
    let dbg: DebugAgentsResponse = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing debug state: {e}"),
    };

    let s = &dbg.summary;
    let mut out = String::from("## Orchestrator state-mismatch diagnostic\n\n");
    out.push_str(&format!(
        "- **Total agents**: {} | **Running**: {} | **WS connected**: {} | **Active workflows**: {}\n",
        s.total_agents, s.running, s.ws_connected, s.active_workflows
    ));

    let mut had_finding = false;

    if !s.running_but_disconnected.is_empty() {
        had_finding = true;
        out.push_str(&format!(
            "\n### 🔴 Running but disconnected ({} agents)\n\n",
            s.running_but_disconnected.len()
        ));
        out.push_str("These agents have status=running but no live WebSocket. They likely cannot receive messages and may need a restart.\n\n");
        for id in &s.running_but_disconnected {
            if let Some(a) = dbg.agents.iter().find(|a| &a.id == id) {
                out.push_str(&format!(
                    "- `{}` **{}** (session: {:?})\n",
                    a.id, a.name, a.session_id
                ));
            } else {
                out.push_str(&format!("- `{id}`\n"));
            }
        }
        out.push_str("\n_Remediation: `restart_agent <id>` for each, or `restart_failed_agents` if these are also in failed state._\n");
    }

    if !s.connected_but_not_running.is_empty() {
        had_finding = true;
        out.push_str(&format!(
            "\n### 🟡 Connected but not running ({} agents)\n\n",
            s.connected_but_not_running.len()
        ));
        out.push_str("These agents have a live WebSocket but are not in running state. The session may be in a transitional state.\n\n");
        for id in &s.connected_but_not_running {
            if let Some(a) = dbg.agents.iter().find(|a| &a.id == id) {
                out.push_str(&format!("- `{}` **{}** status=`{}`\n", a.id, a.name, a.status));
            }
        }
    }

    if !dbg.orphan_connections.is_empty() {
        had_finding = true;
        out.push_str(&format!(
            "\n### 🟠 Orphan WebSocket connections ({})\n\n",
            dbg.orphan_connections.len()
        ));
        out.push_str("WebSockets connected with agent IDs that have no corresponding database record. Usually transient — investigate if persistent.\n\n");
        for id in &dbg.orphan_connections {
            out.push_str(&format!("- `{id}`\n"));
        }
    }

    if !had_finding {
        out.push_str("\n🟢 No state mismatches detected — orchestrator state is consistent.\n");
    }

    out
}

pub async fn run_inspect_queue(
    client: &AgentdClient,
    queue_name: &str,
    peek_limit: Option<u32>,
) -> String {
    let base = client.orchestrator_url();
    let peek_n = peek_limit.unwrap_or(10).clamp(1, 100);

    let stats: QueueStats =
        match client.inner.get(format!("{base}/queues/{queue_name}/stats")).send().await {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(v) => v,
                Err(e) => return format!("Error parsing queue stats: {e}"),
            },
            Ok(r) => return format!("Error: HTTP {} fetching queue stats", r.status()),
            Err(e) => return format!("Error: orchestrator unreachable at {base}: {e}"),
        };

    let tasks: Vec<QueueTask> = match client
        .inner
        .get(format!("{base}/queues/{queue_name}/peek?limit={peek_n}"))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        _ => Vec::new(),
    };

    let mut out = format!("## Queue `{queue_name}`\n\n");
    out.push_str("| Pending | Processing | Completed | Failed | Dead |\n");
    out.push_str("|---------|------------|-----------|--------|------|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n\n",
        stats.pending, stats.processing, stats.completed, stats.failed, stats.dead
    ));

    if stats.pending > 0 && tasks.is_empty() {
        out.push_str("_⚠ Stats show pending tasks but peek returned none — possible permissions or queue-name mismatch._\n");
    } else if tasks.is_empty() {
        out.push_str("_No pending tasks to peek._\n");
    } else {
        out.push_str(&format!("### Next {} pending tasks\n\n", tasks.len()));
        out.push_str("| Priority | Retries | Title | Status | Created |\n");
        out.push_str("|----------|---------|-------|--------|--------|\n");
        for t in &tasks {
            out.push_str(&format!(
                "| {} | {}/{} | {} | {} | {} |\n",
                t.priority,
                t.retry_count,
                t.max_retries,
                truncate(&t.title, 50),
                t.status,
                t.created_at
            ));
        }
    }

    out
}

pub async fn run_get_conversation_summary(client: &AgentdClient, agent_id: &str) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/agents/{agent_id}/conversation/summary");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: orchestrator unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        return format!("Error: HTTP {} fetching conversation summary", resp.status());
    }
    let sum: ConversationSummary = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing summary: {e}"),
    };

    let mut out = format!("## Conversation summary for agent `{}`\n\n", sum.agent_id);
    out.push_str(&format!("- **Total events**: {}\n", sum.total_events));
    out.push_str(&format!("- **Sessions**: {}\n", sum.session_count));
    out.push_str(&format!(
        "- **First event**: {}\n",
        sum.first_event_at.as_deref().unwrap_or("(never)")
    ));
    out.push_str(&format!(
        "- **Last event**: {}\n\n",
        sum.last_event_at.as_deref().unwrap_or("(never)")
    ));

    if sum.event_counts.is_empty() {
        out.push_str("_No event history._\n");
    } else {
        out.push_str("### Event counts by type\n\n");
        out.push_str("| Event type | Count |\n");
        out.push_str("|-----------|-------|\n");
        let mut entries: Vec<(&String, u64)> =
            sum.event_counts.iter().map(|(k, v)| (k, v.as_u64().unwrap_or(0))).collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.1));
        for (k, v) in entries {
            out.push_str(&format!("| `{k}` | {v} |\n"));
        }
    }

    out
}

pub async fn run_list_projects(client: &AgentdClient, limit: Option<u32>) -> String {
    let base = client.orchestrator_url();
    let limit_val = limit.unwrap_or(50).clamp(1, 200);
    let url = format!("{base}/projects?limit={limit_val}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: orchestrator unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        return format!("Error: HTTP {} listing projects", resp.status());
    }
    let page: Paginated<Project> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing projects: {e}"),
    };

    if page.items.is_empty() {
        return "No projects defined.".to_string();
    }

    let mut out = format!("## Projects — {} total\n\n", page.total);
    out.push_str("| ID | Name | Description | Created |\n");
    out.push_str("|-----|------|-------------|--------|\n");
    for p in &page.items {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            p.id,
            p.name,
            truncate(p.description.as_deref().unwrap_or("-"), 60),
            p.created_at
        ));
    }
    out
}

pub async fn run_get_project(client: &AgentdClient, project_id: &str) -> String {
    let base = client.orchestrator_url();
    let url = format!("{base}/projects/{project_id}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: orchestrator unreachable at {base}: {e}"),
    };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Project `{project_id}` not found.");
    }
    if !resp.status().is_success() {
        return format!("Error: HTTP {} fetching project", resp.status());
    }
    let p: ProjectDetail = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing project: {e}"),
    };

    let mut out = format!("## Project: {}\n\n", p.name);
    out.push_str(&format!("- **ID**: `{}`\n", p.id));
    if let Some(ref d) = p.description {
        out.push_str(&format!("- **Description**: {d}\n"));
    }
    out.push_str(&format!("- **Agents**: {}\n", p.agent_count));
    out.push_str(&format!("- **Workflows**: {}\n", p.workflow_count));
    out.push_str(&format!("- **Created**: {}\n", p.created_at));
    out.push_str(&format!("- **Updated**: {}\n", p.updated_at));
    out
}
