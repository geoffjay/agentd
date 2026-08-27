//! Diagnostic tool implementations for agentd-mcp.
//!
//! Each `run_*` function performs multi-service aggregation and returns a
//! human-readable markdown report with severity-tagged issues and actionable
//! remediation steps that reference other MCP tools.

use crate::client::AgentdClient;
use std::fmt::Write;

// ── Helpers ────────────────────────────────────────────────────────────────

/// Test if a service `/health` endpoint responds successfully.
async fn service_reachable(client: &AgentdClient, url: &str) -> bool {
    client.inner.get(url).send().await.map(|r| r.status().is_success()).unwrap_or(false)
}

/// Fetch a JSON value from a URL, returning None on any error.
async fn fetch_json(client: &AgentdClient, url: &str) -> Option<serde_json::Value> {
    client.get::<serde_json::Value>(url).await.ok()
}

fn str_field<'a>(v: &'a serde_json::Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("unknown")
}

fn u64_field(v: &serde_json::Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or(0)
}

// ── diagnose_agent ─────────────────────────────────────────────────────────

/// Run a comprehensive multi-point diagnostic on a specific agent.
pub async fn run_diagnose_agent(client: &AgentdClient, agent_id: &str) -> String {
    let mut report = String::new();
    let base = client.orchestrator_url();

    // ── 1. Fetch agent details ──────────────────────────────────────────
    let agent_url = format!("{base}/agents/{agent_id}");
    let agent = match fetch_json(client, &agent_url).await {
        Some(v) => v,
        None => {
            writeln!(report, "# Agent Diagnostic: {agent_id}").ok();
            writeln!(report, "\n## 🔴 Error").ok();
            writeln!(report, "- Could not fetch agent. Verify the agent ID and that the orchestrator is running.").ok();
            writeln!(report, "  → Check connectivity: `check_service_health`").ok();
            return report;
        }
    };

    let name = str_field(&agent, "name");
    let status = str_field(&agent, "status");
    let activity = str_field(&agent, "activity");
    let updated_at = str_field(&agent, "updated_at");
    let backend_type = str_field(&agent, "backend_type");
    let session_id = agent["session_id"].as_str();
    let pid = agent["pid"].as_u64();

    writeln!(report, "# Agent Diagnostic: {name}").ok();
    writeln!(report, "- **ID:** `{agent_id}`").ok();
    writeln!(report, "- **Status:** `{status}`").ok();
    writeln!(report, "- **Activity:** `{activity}`").ok();
    writeln!(report, "- **Backend:** `{backend_type}`").ok();
    if let Some(sid) = session_id {
        writeln!(report, "- **Session ID:** `{sid}`").ok();
    }
    if let Some(p) = pid {
        writeln!(report, "- **PID:** {p}").ok();
    }
    writeln!(report, "- **Last Updated:** {updated_at}").ok();

    // ── 2. Status analysis ──────────────────────────────────────────────
    let mut critical: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    match status {
        "Failed" => {
            critical
                .push("Agent is in **Failed** state.\n  → Restart it: `restart_agent`".to_string());
        }
        "Stopped" => {
            warnings.push("Agent is stopped.\n  → Start it if expected to run.".to_string());
        }
        "Pending" => {
            warnings.push("Agent is still in Pending state — may be slow to start.".to_string());
        }
        "Running" => {
            info.push(format!("Agent is running ({activity})."));
        }
        other => {
            warnings.push(format!("Unknown status: `{other}`"));
        }
    }

    // ── 3. Pending approval backlog ─────────────────────────────────────
    let approvals_url = format!("{base}/agents/{agent_id}/approvals?status=Pending&limit=10");
    if let Some(resp) = fetch_json(client, &approvals_url).await {
        let total = u64_field(&resp, "total");
        if total > 0 {
            let mut msg =
                format!("Agent has **{total}** pending tool approval(s) — it may be blocked.\n");
            if let Some(items) = resp["items"].as_array() {
                for item in items.iter().take(5) {
                    let tool = str_field(item, "tool_name");
                    let created = str_field(item, "created_at");
                    writeln!(msg, "  - `{tool}` (requested {created})").ok();
                }
            }
            write!(msg, "  → Use `approve_tool` or `deny_tool` to unblock.").ok();
            warnings.push(msg);
        }
    } else {
        info.push(
            "Could not fetch approval backlog (orchestrator endpoint unavailable).".to_string(),
        );
    }

    // ── 4. Backend-specific health (wrap service cross-check) ───────────
    if let Some(sid) = session_id {
        let wrap_url = format!("{}/sessions/{sid}", client.wrap_url());
        let session_exists = match client.inner.get(&wrap_url).send().await {
            Ok(r) => Some(r.status().is_success()),
            Err(_) => None,
        };
        match (status, session_exists) {
            ("Running", Some(false)) => {
                critical.push(format!(
                    "Agent status is **Running** but the {backend_type} session `{sid}` does not \
                     exist in the wrap service. The backend session has died but the orchestrator \
                     has not noticed.\n  → `restart_agent`"
                ));
            }
            ("Running", Some(true)) => {
                info.push(format!("{backend_type} session `{sid}` is alive ✓"));
            }
            ("Stopped" | "Failed", Some(true)) => {
                warnings.push(format!(
                    "Agent is {status} but a backend session `{sid}` still exists. \
                     Consider `terminate_agent` to clean up the lingering session."
                ));
            }
            (_, None) => {
                info.push(
                    "Wrap service unreachable — could not verify backend session existence."
                        .to_string(),
                );
            }
            _ => {}
        }

        match backend_type {
            "subprocess" => {
                info.push(
                    "Backend is **subprocess** — uses stdin/stdout NDJSON. If the agent appears \
                     hung, the subprocess may have crashed; check its PID."
                        .to_string(),
                );
            }
            "pty" => {
                info.push("Backend is **pty** — direct PTY session.".to_string());
            }
            "tmux" => {
                info.push(
                    "Backend is **tmux** — session can be attached for inspection.".to_string(),
                );
            }
            _ => {}
        }
    }

    // ── 5. Usage / last activity ────────────────────────────────────────
    let usage_url = format!("{base}/agents/{agent_id}/usage");
    if let Some(usage) = fetch_json(client, &usage_url).await {
        if let Some(current) = usage["current_session"].as_object() {
            let turns = current.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0);
            let cost = current.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
            info.push(format!("Current session: {turns} turns, ${cost:.4} USD used."));
        } else {
            info.push("No active session usage data.".to_string());
        }
    }

    // ── 6. Render report ────────────────────────────────────────────────
    if !critical.is_empty() {
        writeln!(report, "\n## 🔴 Critical").ok();
        for item in &critical {
            writeln!(report, "- {item}").ok();
        }
    }
    if !warnings.is_empty() {
        writeln!(report, "\n## 🟡 Warnings").ok();
        for item in &warnings {
            writeln!(report, "- {item}").ok();
        }
    }
    if !info.is_empty() {
        writeln!(report, "\n## 🟢 Info").ok();
        for item in &info {
            writeln!(report, "- {item}").ok();
        }
    }
    if critical.is_empty() && warnings.is_empty() {
        writeln!(report, "\n✅ No issues detected.").ok();
    }

    report
}

// ── diagnose_workflow ──────────────────────────────────────────────────────

/// Analyze workflow configuration and dispatch history.
pub async fn run_diagnose_workflow(client: &AgentdClient, workflow_id: &str) -> String {
    let mut report = String::new();
    let base = client.orchestrator_url();

    // ── 1. Fetch workflow ───────────────────────────────────────────────
    let wf_url = format!("{base}/workflows/{workflow_id}");
    let wf = match fetch_json(client, &wf_url).await {
        Some(v) => v,
        None => {
            writeln!(report, "# Workflow Diagnostic: {workflow_id}").ok();
            writeln!(report, "\n## 🔴 Error").ok();
            writeln!(report, "- Could not fetch workflow. Verify the workflow ID.").ok();
            return report;
        }
    };

    let wf_name = str_field(&wf, "name");
    let agent_id = str_field(&wf, "agent_id");
    let enabled = wf["enabled"].as_bool().unwrap_or(false);
    let trigger_type =
        wf["trigger_type"].as_str().or_else(|| wf["source"]["type"].as_str()).unwrap_or("unknown");

    writeln!(report, "# Workflow Diagnostic: {wf_name}").ok();
    writeln!(report, "- **ID:** `{workflow_id}`").ok();
    writeln!(report, "- **Agent:** `{agent_id}`").ok();
    writeln!(report, "- **Trigger:** `{trigger_type}`").ok();
    writeln!(report, "- **Enabled:** {enabled}").ok();

    let mut critical: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    if !enabled {
        warnings.push("Workflow is **disabled** and will not trigger automatically.".to_string());
    }

    // Trigger-specific notes
    match trigger_type {
        "cron" => {
            let schedule = wf["source"]["schedule"].as_str().unwrap_or("?");
            info.push(format!(
                "Cron schedule: `{schedule}`. If dispatches aren't happening, verify the expression \
                 parses correctly and the orchestrator's scheduler is running."
            ));
        }
        "delay" => {
            info.push(
                "Delay-triggered: fires once after a configured delay. After firing, will not re-trigger \
                 unless manually reset."
                    .to_string(),
            );
        }
        "webhook" => {
            info.push(
                "Webhook-triggered: fires on inbound POSTs. Check the webhook secret + signature \
                 validation if expected events are not arriving."
                    .to_string(),
            );
        }
        "agent_lifecycle" => {
            let event = wf["source"]["event"].as_str().unwrap_or("?");
            info.push(format!(
                "AgentLifecycle-triggered on `{event}`. Fires when the upstream agent emits the event."
            ));
        }
        "agent_idle" => {
            info.push(
                "AgentIdle-triggered: fires after the workflow's agent has been idle for a configured \
                 period. Verify the agent is reaching idle (not stuck in tool approvals)."
                    .to_string(),
            );
        }
        "dispatch_result" => {
            let upstream = wf["source"]["upstream_workflow_id"].as_str().unwrap_or("?");
            info.push(format!(
                "DispatchResult-triggered: fires on completion of upstream workflow `{upstream}`. \
                 Diagnose upstream first if downstream isn't firing."
            ));
        }
        "manual" => {
            info.push(
                "Manual trigger: only fires when explicitly dispatched via the API.".to_string(),
            );
        }
        "github_issues" | "github_pull_requests" => {
            info.push(format!(
                "Polling trigger (`{trigger_type}`). Verify the orchestrator can reach the GitHub \
                 API and the configured token has access."
            ));
        }
        "linear_issues" => {
            info.push("Polling Linear. Verify API key and team filter configuration.".to_string());
        }
        _ => {}
    }

    // ── 2. Check associated agent ───────────────────────────────────────
    let agent_url = format!("{base}/agents/{agent_id}");
    if let Some(agent) = fetch_json(client, &agent_url).await {
        let status = str_field(&agent, "status");
        let agent_name = str_field(&agent, "name");
        match status {
            "Failed" => critical.push(format!(
                "Associated agent `{agent_name}` is **Failed**.\n  → Fix the agent first: `diagnose_agent({agent_id})`"
            )),
            "Stopped" => warnings.push(format!(
                "Associated agent `{agent_name}` is stopped — workflow will not dispatch."
            )),
            "Running" => info.push(format!("Agent `{agent_name}` is running ✓")),
            other => warnings.push(format!("Agent status is `{other}`")),
        }
    } else {
        critical.push(format!(
            "Could not reach agent `{agent_id}`. Orchestrator may be down.\n  → `check_service_health`"
        ));
    }

    // ── 3. Analyse dispatch history ────────────────────────────────────
    let hist_url = format!("{base}/workflows/{workflow_id}/history?limit=20");
    if let Some(resp) = fetch_json(client, &hist_url).await {
        let total = u64_field(&resp, "total");
        if total == 0 {
            info.push("No dispatch history yet.".to_string());
        } else {
            let items = resp["items"].as_array().cloned().unwrap_or_default();
            let dispatched = items.len() as u64;
            let completed =
                items.iter().filter(|d| str_field(d, "status") == "Completed").count() as u64;
            let failed = items.iter().filter(|d| str_field(d, "status") == "Failed").count() as u64;
            let success_rate = (completed * 100).checked_div(dispatched).unwrap_or(0);

            info.push(format!(
                "Last {dispatched} dispatches: {completed} completed, {failed} failed \
                 ({success_rate}% success rate)."
            ));

            if success_rate < 50 && dispatched >= 5 {
                warnings.push(format!(
                    "Low success rate ({success_rate}%) over last {dispatched} dispatches. \
                     Check agent logs or prompt template configuration."
                ));
            }

            // Consecutive failures
            let consecutive_failures =
                items.iter().take_while(|d| str_field(d, "status") == "Failed").count();
            if consecutive_failures >= 3 {
                critical.push(format!(
                    "**{consecutive_failures} consecutive failures** detected. \
                     Workflow may be in a broken state.\n  → Review prompt template or trigger configuration."
                ));
            }
        }
    } else {
        info.push("Could not fetch dispatch history.".to_string());
    }

    // ── 4. Render ───────────────────────────────────────────────────────
    if !critical.is_empty() {
        writeln!(report, "\n## 🔴 Critical").ok();
        for item in &critical {
            writeln!(report, "- {item}").ok();
        }
    }
    if !warnings.is_empty() {
        writeln!(report, "\n## 🟡 Warnings").ok();
        for item in &warnings {
            writeln!(report, "- {item}").ok();
        }
    }
    if !info.is_empty() {
        writeln!(report, "\n## 🟢 Info").ok();
        for item in &info {
            writeln!(report, "- {item}").ok();
        }
    }
    if critical.is_empty() && warnings.is_empty() {
        writeln!(report, "\n✅ Workflow looks healthy.").ok();
    }

    report
}

// ── diagnose_system ────────────────────────────────────────────────────────

/// Full system health overview with prioritised issues.
pub async fn run_diagnose_system(client: &AgentdClient) -> String {
    let mut report = String::new();
    let orch = client.orchestrator_url();
    let mon = client.monitor_url();
    let notif = client.notify_url();

    let mut critical: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    writeln!(report, "# System Diagnostic Report").ok();

    // ── 1. Orchestrator health ──────────────────────────────────────────
    let orch_health_url = format!("{orch}/health");
    if !service_reachable(client, &orch_health_url).await {
        critical.push(
            "**Orchestrator is unreachable** — agent management unavailable.\n  \
             → Check that `agentd-orchestrator` is running."
                .to_string(),
        );
    } else {
        info.push("Orchestrator: reachable ✓".to_string());

        // Failed agents
        let failed_url = format!("{orch}/agents?status=Failed&limit=50");
        if let Some(resp) = fetch_json(client, &failed_url).await {
            let total = u64_field(&resp, "total");
            if total > 0 {
                let mut msg = format!("**{total} agent(s) in Failed state:**\n");
                if let Some(items) = resp["items"].as_array() {
                    for item in items.iter().take(10) {
                        let name = str_field(item, "name");
                        let id = str_field(item, "id");
                        writeln!(msg, "  - `{name}` (`{id}`)").ok();
                    }
                }
                write!(msg, "  → Use `diagnose_agent` or `restart_agent` for each.").ok();
                critical.push(msg);
            } else {
                info.push("No agents in Failed state ✓".to_string());
            }
        }

        // Pending approvals
        let approvals_url = format!("{orch}/approvals?status=Pending&limit=50");
        if let Some(resp) = fetch_json(client, &approvals_url).await {
            let total = u64_field(&resp, "total");
            if total > 5 {
                warnings.push(format!(
                    "**{total} pending tool approvals** — agents may be blocked.\n  \
                     → Review with `approve_tool` / `deny_tool`."
                ));
            } else if total > 0 {
                info.push(format!("{total} pending tool approval(s)."));
            }
        }
    }

    // ── 2. Monitor service (graceful degradation) ───────────────────────
    let mon_status_url = format!("{mon}/status");
    match client.get::<serde_json::Value>(&mon_status_url).await {
        Ok(status) => {
            let health = str_field(&status, "status");
            match health {
                "Critical" => {
                    critical.push(
                        "Monitor reports **Critical** system health. Check resource usage."
                            .to_string(),
                    );
                }
                "Degraded" => {
                    warnings.push("Monitor reports **Degraded** system health.".to_string());
                }
                _ => {
                    info.push(format!("System resources: {health} ✓"));
                }
            }
            // Surface alerts
            if let Some(alerts) = status["alerts"].as_array() {
                for alert in alerts {
                    let metric = str_field(alert, "metric");
                    let msg = str_field(alert, "message");
                    warnings.push(format!("Monitor alert — `{metric}`: {msg}"));
                }
            }
        }
        Err(_) => {
            info.push(
                "Monitor service unavailable — resource metrics not included in this report."
                    .to_string(),
            );
        }
    }

    // ── 3. Notify service ───────────────────────────────────────────────
    let notify_count_url = format!("{notif}/notifications/count");
    match client.get::<serde_json::Value>(&notify_count_url).await {
        Ok(count) => {
            let total = u64_field(&count, "total");
            if total > 0 {
                // Find pending/urgent count
                let pending = count["by_status"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|s| s["status"].as_str() == Some("Pending"))
                    .map(|s| s["count"].as_u64().unwrap_or(0))
                    .sum::<u64>();
                if pending > 10 {
                    warnings.push(format!(
                        "**{pending} pending notifications** — review with `list_notifications`."
                    ));
                } else {
                    info.push(format!("{pending} pending notification(s)."));
                }
            }
        }
        Err(_) => {
            info.push("Notify service unavailable — notification counts not included.".to_string());
        }
    }

    // ── 4. Render ───────────────────────────────────────────────────────
    if !critical.is_empty() {
        writeln!(report, "\n## 🔴 Critical Issues").ok();
        for item in &critical {
            writeln!(report, "- {item}").ok();
        }
    }
    if !warnings.is_empty() {
        writeln!(report, "\n## 🟡 Warnings").ok();
        for item in &warnings {
            writeln!(report, "- {item}").ok();
        }
    }
    if !info.is_empty() {
        writeln!(report, "\n## 🟢 Info").ok();
        for item in &info {
            writeln!(report, "- {item}").ok();
        }
    }

    if critical.is_empty() && warnings.is_empty() {
        writeln!(report, "\n✅ System appears healthy. No critical issues found.").ok();
    } else {
        writeln!(
            report,
            "\n---\n\
             *Run `diagnose_agent <id>` or `diagnose_workflow <id>` for deeper analysis. \
             Use `check_service_health` if services appear unreachable.*"
        )
        .ok();
    }

    report
}
