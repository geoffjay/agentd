//! Service health and system metrics tool implementations.
//!
//! Health checks fan out concurrently across all agentd services using a
//! short (3 s) timeout so that unresponsive services never block the tool.
//! The monitor service is optional — its absence produces a graceful
//! degraded response rather than an error.

use crate::client::AgentdClient;
use reqwest::Client;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::time::{Duration, Instant};

// ── Per-service health record ──────────────────────────────────────────────

#[derive(Debug)]
struct ServiceHealth {
    name: &'static str,
    url: String,
    status: HealthStatus,
    response_ms: Option<u64>,
}

#[derive(Debug, PartialEq)]
enum HealthStatus {
    Healthy,
    Unreachable,
    Error(u16),
}

impl HealthStatus {
    fn icon(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "✅",
            HealthStatus::Unreachable => "❌",
            HealthStatus::Error(_) => "⚠️",
        }
    }
    fn label(&self) -> String {
        match self {
            HealthStatus::Healthy => "healthy".to_string(),
            HealthStatus::Unreachable => "unreachable".to_string(),
            HealthStatus::Error(code) => format!("error (HTTP {code})"),
        }
    }
}

// ── Timeout client ─────────────────────────────────────────────────────────

fn timeout_client() -> Client {
    Client::builder().timeout(Duration::from_secs(3)).build().unwrap_or_default()
}

// ── Single-service health check ────────────────────────────────────────────

async fn probe(client: &Client, name: &'static str, base_url: &str) -> ServiceHealth {
    let url = format!("{base_url}/health");
    let start = Instant::now();
    match client.get(&url).send().await {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let status_code = resp.status();
            if status_code.is_success() {
                ServiceHealth {
                    name,
                    url: base_url.to_string(),
                    status: HealthStatus::Healthy,
                    response_ms: Some(elapsed),
                }
            } else {
                ServiceHealth {
                    name,
                    url: base_url.to_string(),
                    status: HealthStatus::Error(status_code.as_u16()),
                    response_ms: Some(elapsed),
                }
            }
        }
        Err(_) => ServiceHealth {
            name,
            url: base_url.to_string(),
            status: HealthStatus::Unreachable,
            response_ms: None,
        },
    }
}

// ── Render helpers ─────────────────────────────────────────────────────────

fn render_health_table(results: &[ServiceHealth]) -> String {
    let mut out = String::new();
    writeln!(out, "| Service | Status | Response | URL |").ok();
    writeln!(out, "|---------|--------|----------|-----|").ok();
    for r in results {
        let icon = r.status.icon();
        let label = r.status.label();
        let ms = r.response_ms.map(|m| format!("{m} ms")).unwrap_or_else(|| "—".to_string());
        writeln!(out, "| {icon} **{}** | {label} | {ms} | `{}` |", r.name, r.url).ok();
    }
    out
}

fn overall_summary(results: &[ServiceHealth]) -> &'static str {
    let any_unreachable = results.iter().any(|r| r.status == HealthStatus::Unreachable);
    let any_error = results.iter().any(|r| matches!(r.status, HealthStatus::Error(_)));
    if any_unreachable {
        "🔴 One or more services are unreachable."
    } else if any_error {
        "🟡 All services responded, but some returned errors."
    } else {
        "🟢 All services are healthy."
    }
}

// ── check_service_health ───────────────────────────────────────────────────

/// Concurrently check health of all agentd services.
pub async fn run_check_service_health(client: &AgentdClient) -> String {
    let http = timeout_client();

    // Fan out all health checks concurrently
    let (r_orch, r_comm, r_mem, r_notify, r_ask, r_wrap, r_monitor, r_hook) = tokio::join!(
        probe(&http, "orchestrator", client.orchestrator_url()),
        probe(&http, "communicate", client.communicate_url()),
        probe(&http, "memory", client.memory_url()),
        probe(&http, "notify", client.notify_url()),
        probe(&http, "ask", client.ask_url()),
        probe(&http, "wrap", client.wrap_url()),
        probe(&http, "monitor", client.monitor_url()),
        probe(&http, "hook", client.hook_url()),
    );

    let results = [r_orch, r_comm, r_mem, r_notify, r_ask, r_wrap, r_monitor, r_hook];
    let summary = overall_summary(&results);

    let mut out = String::new();
    writeln!(out, "# Service Health Report\n").ok();
    out.push_str(&render_health_table(&results));
    writeln!(out, "\n**Overall:** {summary}").ok();
    out
}

// ── check_single_service ───────────────────────────────────────────────────

/// Check health of a specific named service.
pub async fn run_check_single_service(client: &AgentdClient, service: &str) -> String {
    let (name, base_url): (&'static str, &str) = match service.to_lowercase().as_str() {
        "orchestrator" => ("orchestrator", client.orchestrator_url()),
        "communicate" | "comm" => ("communicate", client.communicate_url()),
        "memory" | "mem" => ("memory", client.memory_url()),
        "notify" | "notification" => ("notify", client.notify_url()),
        "ask" => ("ask", client.ask_url()),
        "wrap" => ("wrap", client.wrap_url()),
        "monitor" => ("monitor", client.monitor_url()),
        "hook" => ("hook", client.hook_url()),
        other => {
            return format!(
                "🔴 Unknown service `{other}`.\n\
                 Valid names: orchestrator, communicate, memory, notify, ask, wrap, monitor, hook"
            );
        }
    };

    let http = timeout_client();
    let r = probe(&http, name, base_url).await;

    let mut out = String::new();
    writeln!(out, "# Health: {name}\n").ok();
    out.push_str(&render_health_table(&[r]));
    out
}

// ── get_system_metrics ─────────────────────────────────────────────────────

/// Fetch system metrics from the monitor service.
pub async fn run_get_system_metrics(client: &AgentdClient) -> String {
    let metrics_url = format!("{}/metrics", client.monitor_url());
    let status_url = format!("{}/status", client.monitor_url());

    match client.get::<serde_json::Value>(&metrics_url).await {
        Ok(m) => {
            let collected_at = m["collected_at"].as_str().unwrap_or("unknown");

            // CPU
            let cpu_pct = m["cpu"]["usage_percent"].as_f64().unwrap_or(0.0);
            let core_count = m["cpu"]["core_count"].as_u64().unwrap_or(0);

            // Memory
            let mem_used = m["memory"]["used_bytes"].as_u64().unwrap_or(0);
            let mem_total = m["memory"]["total_bytes"].as_u64().unwrap_or(1);
            let mem_pct = m["memory"]["usage_percent"].as_f64().unwrap_or(0.0);

            // Load average
            let load_1 = m["load_average"]["one"].as_f64().unwrap_or(0.0);
            let load_5 = m["load_average"]["five"].as_f64().unwrap_or(0.0);
            let load_15 = m["load_average"]["fifteen"].as_f64().unwrap_or(0.0);

            let mut out = String::new();
            writeln!(out, "# System Metrics").ok();
            writeln!(out, "*Collected at: {collected_at}*\n").ok();

            writeln!(out, "## CPU").ok();
            writeln!(out, "- Usage: **{cpu_pct:.1}%** ({core_count} cores)").ok();
            let cpu_bar = progress_bar(cpu_pct as u32, 100);
            writeln!(out, "- {cpu_bar}").ok();

            writeln!(out, "\n## Memory").ok();
            writeln!(
                out,
                "- Usage: **{mem_pct:.1}%** ({} / {})",
                format_bytes(mem_used),
                format_bytes(mem_total)
            )
            .ok();
            let mem_bar = progress_bar(mem_pct as u32, 100);
            writeln!(out, "- {mem_bar}").ok();

            writeln!(out, "\n## Load Average").ok();
            writeln!(
                out,
                "- 1 min: **{load_1:.2}**  5 min: **{load_5:.2}**  15 min: **{load_15:.2}**"
            )
            .ok();

            // Disks
            if let Some(disks) = m["disks"].as_array() {
                if !disks.is_empty() {
                    writeln!(out, "\n## Disks").ok();
                    for disk in disks {
                        let mount = disk["mount_point"].as_str().unwrap_or("?");
                        let used = disk["used_bytes"].as_u64().unwrap_or(0);
                        let total = disk["total_bytes"].as_u64().unwrap_or(1);
                        let pct = disk["usage_percent"].as_f64().unwrap_or(0.0);
                        writeln!(
                            out,
                            "- `{mount}`: {pct:.1}% ({} / {})",
                            format_bytes(used),
                            format_bytes(total)
                        )
                        .ok();
                    }
                }
            }

            // Alerts from /status endpoint (best-effort)
            if let Ok(status) = client.get::<serde_json::Value>(&status_url).await {
                if let Some(alerts) = status["alerts"].as_array() {
                    if !alerts.is_empty() {
                        writeln!(out, "\n## ⚠️ Active Alerts").ok();
                        for alert in alerts {
                            let metric = alert["metric"].as_str().unwrap_or("?");
                            let msg = alert["message"].as_str().unwrap_or("?");
                            writeln!(out, "- `{metric}`: {msg}").ok();
                        }
                    }
                }
            }

            out
        }
        Err(_) => "# System Metrics\n\n\
             🟡 Monitor service is unavailable — system metrics cannot be retrieved.\n\
             → Check monitor status: `check_single_service(\"monitor\")`"
            .to_string(),
    }
}

// ── get_prometheus_metrics ─────────────────────────────────────────────────

/// Fetch and parse key Prometheus metrics from a service.
pub async fn run_get_prometheus_metrics(client: &AgentdClient, service: Option<&str>) -> String {
    let svc_input = service.unwrap_or("orchestrator").to_lowercase();
    let (svc_name, base_url): (&str, &str) = match svc_input.as_str() {
        "orchestrator" => ("orchestrator", client.orchestrator_url()),
        "notify" | "notification" => ("notify", client.notify_url()),
        "memory" | "mem" => ("memory", client.memory_url()),
        "communicate" | "comm" => ("communicate", client.communicate_url()),
        "monitor" => ("monitor", client.monitor_url()),
        "ask" => ("ask", client.ask_url()),
        "wrap" => ("wrap", client.wrap_url()),
        "hook" => ("hook", client.hook_url()),
        other => {
            return format!(
                "🔴 Unknown service `{other}`.\n\
                 Valid: orchestrator, notify, memory, communicate, monitor, ask, wrap, hook"
            );
        }
    };

    let url = format!("{base_url}/metrics");
    match client.inner.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.text().await {
            Ok(text) => {
                let parsed = parse_prometheus_text(&text);
                render_prometheus_report(svc_name, &parsed)
            }
            Err(e) => format!("🔴 Failed to read metrics response from {svc_name}: {e}"),
        },
        Ok(resp) => format!("🔴 {svc_name} /metrics returned HTTP {}.", resp.status().as_u16()),
        Err(e) => format!("🔴 Could not reach {svc_name} for Prometheus metrics: {e}"),
    }
}

/// Parse Prometheus text-format into a map of metric_name → summed value.
fn parse_prometheus_text(text: &str) -> BTreeMap<String, f64> {
    let mut metrics: BTreeMap<String, f64> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Split off the value (last whitespace-delimited token)
        if let Some((name_part, value_str)) = line.rsplit_once(' ') {
            if let Ok(value) = value_str.trim().parse::<f64>() {
                // Strip labels: everything before '{'
                let bare_name = name_part.split('{').next().unwrap_or(name_part).trim().to_string();
                metrics.entry(bare_name).and_modify(|e| *e += value).or_insert(value);
            }
        }
    }
    metrics
}

/// Render a human-readable summary of the key operational metrics.
fn render_prometheus_report(svc: &str, metrics: &BTreeMap<String, f64>) -> String {
    let mut out = String::new();
    writeln!(out, "# Prometheus Metrics: {svc}\n").ok();

    // Key metrics to highlight per service
    let key_metrics = [
        // Orchestrator
        "agents_created_total",
        "agents_active",
        "agents_terminated_total",
        "agents_restarted_total",
        "agent_dispatches_total",
        "agent_messages_sent_total",
        "websocket_connections_active",
        "websocket_connections_total",
        "approvals_pending",
        "approvals_resolved_total",
        "context_clears_total",
        "workflows_active",
        // Notify
        "notifications_created_total",
        "notifications_pending",
        "notifications_responded_total",
        "notifications_dismissed_total",
        // Memory
        "memories_created_total",
        "memories_searched_total",
        "memories_deleted_total",
        // Communicate
        "rooms_created_total",
        "rooms_active",
        "messages_sent_total",
        // Monitor
        "system_cpu_usage_percent",
        "system_memory_used_bytes",
        "system_memory_total_bytes",
        "collections_total",
        // General
        "http_requests_total",
        "service_info",
    ];

    writeln!(out, "## Key Metrics\n").ok();
    let mut found_any = false;
    for &key in &key_metrics {
        if let Some(&val) = metrics.get(key) {
            writeln!(out, "- `{key}`: **{val}**").ok();
            found_any = true;
        }
    }
    if !found_any {
        writeln!(out, "*No recognised key metrics found in response.*").ok();
    }

    // Show a sample of remaining metrics (up to 20)
    let others: Vec<_> =
        metrics.iter().filter(|(k, _)| !key_metrics.contains(&k.as_str())).take(20).collect();

    if !others.is_empty() {
        writeln!(out, "\n## Other Metrics (sample)\n").ok();
        for (name, val) in &others {
            writeln!(out, "- `{name}`: {val}").ok();
        }
        if metrics.len() > key_metrics.len() + others.len() {
            writeln!(
                out,
                "\n*…and {} more. Fetch raw `/metrics` for the full list.*",
                metrics.len().saturating_sub(key_metrics.len() + others.len())
            )
            .ok();
        }
    }

    out
}

// ── Formatting utilities ───────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    const GIB: u64 = 1 << 30;
    const MIB: u64 = 1 << 20;
    const KIB: u64 = 1 << 10;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn progress_bar(value: u32, max: u32) -> String {
    let pct = value.min(max) as f64 / max as f64;
    let filled = (pct * 20.0).round() as usize;
    let empty = 20usize.saturating_sub(filled);
    format!("[{}{}] {}%", "█".repeat(filled), "░".repeat(empty), value)
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GiB");
    }

    #[test]
    fn test_progress_bar() {
        assert!(progress_bar(50, 100).contains("██████████"));
        assert!(progress_bar(0, 100).contains("░░░░░░░░░░"));
        assert!(progress_bar(100, 100).contains("████████████████████"));
    }

    #[test]
    fn test_parse_prometheus_text() {
        let input = "\
# HELP agents_active Currently active agents
# TYPE agents_active gauge
agents_active 3
# HELP http_requests_total Total HTTP requests
# TYPE http_requests_total counter
http_requests_total{method=\"GET\"} 42
http_requests_total{method=\"POST\"} 8
";
        let metrics = parse_prometheus_text(input);
        assert_eq!(metrics.get("agents_active"), Some(&3.0));
        assert_eq!(metrics.get("http_requests_total"), Some(&50.0)); // 42 + 8
    }

    #[test]
    fn test_overall_summary_all_healthy() {
        let results = vec![ServiceHealth {
            name: "test",
            url: "http://localhost".to_string(),
            status: HealthStatus::Healthy,
            response_ms: Some(5),
        }];
        assert_eq!(overall_summary(&results), "🟢 All services are healthy.");
    }

    #[test]
    fn test_overall_summary_unreachable() {
        let results = vec![ServiceHealth {
            name: "test",
            url: "http://localhost".to_string(),
            status: HealthStatus::Unreachable,
            response_ms: None,
        }];
        assert_eq!(overall_summary(&results), "🔴 One or more services are unreachable.");
    }
}
