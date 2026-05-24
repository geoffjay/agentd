//! Curated PromQL queries shown in the metrics picker.
//!
//! Sourced from `infra/grafana/dashboards/*.json` and the metrics emitted by
//! the agentd service crates. Categories group related queries for the picker
//! list ordering.

pub struct PredefinedQuery {
    pub category: &'static str,
    pub name: &'static str,
    pub query: &'static str,
}

pub const PREDEFINED_QUERIES: &[PredefinedQuery] = &[
    // Liveness
    PredefinedQuery {
        category: "Liveness",
        name: "All services up",
        query: "up",
    },
    PredefinedQuery {
        category: "Liveness",
        name: "Down services",
        query: "up == 0",
    },

    // Orchestrator state
    PredefinedQuery {
        category: "Orchestrator",
        name: "Active agents",
        query: "agents_active",
    },
    PredefinedQuery {
        category: "Orchestrator",
        name: "Active workflows",
        query: "workflows_active",
    },
    PredefinedQuery {
        category: "Orchestrator",
        name: "Pending approvals",
        query: "approvals_pending",
    },
    PredefinedQuery {
        category: "Orchestrator",
        name: "Queued messages",
        query: "messages_queued",
    },
    PredefinedQuery {
        category: "Orchestrator",
        name: "WebSocket connections",
        query: "websocket_connections_active",
    },

    // Activity rates
    PredefinedQuery {
        category: "Activity",
        name: "Agent create rate (5m)",
        query: "rate(agents_created_total[5m])",
    },
    PredefinedQuery {
        category: "Activity",
        name: "Agent terminate rate (5m)",
        query: "rate(agents_terminated_total[5m])",
    },
    PredefinedQuery {
        category: "Activity",
        name: "Workflow dispatch rate (5m)",
        query: "sum by (status) (rate(workflow_dispatches_total[5m]))",
    },
    PredefinedQuery {
        category: "Activity",
        name: "Approval decisions (5m)",
        query: "sum by (decision) (rate(approvals_resolved_total[5m]))",
    },
    PredefinedQuery {
        category: "Activity",
        name: "Agent messages by mode (5m)",
        query: "sum by (mode) (rate(agent_messages_sent_total[5m]))",
    },

    // Latency
    PredefinedQuery {
        category: "Latency",
        name: "HTTP p99 by service (5m)",
        query: "histogram_quantile(0.99, sum by (le, job) (rate(http_request_duration_seconds_bucket[5m])))",
    },
    PredefinedQuery {
        category: "Latency",
        name: "HTTP p50 by service (5m)",
        query: "histogram_quantile(0.50, sum by (le, job) (rate(http_request_duration_seconds_bucket[5m])))",
    },

    // Errors
    PredefinedQuery {
        category: "Errors",
        name: "HTTP 5xx rate by service",
        query: "sum by (job) (rate(http_requests_total{status=~\"5..\"}[5m]))",
    },
    PredefinedQuery {
        category: "Errors",
        name: "HTTP 4xx rate by service",
        query: "sum by (job) (rate(http_requests_total{status=~\"4..\"}[5m]))",
    },
    PredefinedQuery {
        category: "Errors",
        name: "Failed workflow dispatches (5m)",
        query: "sum(rate(workflow_dispatches_total{status=\"failed\"}[5m]))",
    },
    PredefinedQuery {
        category: "Errors",
        name: "Dropped messages (5m)",
        query: "rate(messages_dropped[5m])",
    },

    // Memory service
    PredefinedQuery {
        category: "Memory",
        name: "Search rate (5m)",
        query: "rate(memories_searched_total[5m])",
    },
    PredefinedQuery {
        category: "Memory",
        name: "Memories deleted (5m)",
        query: "rate(memories_deleted_total[5m])",
    },

    // Notify
    PredefinedQuery {
        category: "Notify",
        name: "Pending notifications",
        query: "notifications_pending",
    },
    PredefinedQuery {
        category: "Notify",
        name: "Notification create rate (5m)",
        query: "rate(notifications_created_total[5m])",
    },

    // Communicate
    PredefinedQuery {
        category: "Communicate",
        name: "Active rooms",
        query: "rooms_active",
    },

    // System
    PredefinedQuery {
        category: "System",
        name: "CPU usage",
        query: "system_cpu_usage_percent",
    },
    PredefinedQuery {
        category: "System",
        name: "Memory used (bytes)",
        query: "system_memory_used_bytes",
    },
    PredefinedQuery {
        category: "System",
        name: "Disk usage by mount",
        query: "system_disk_usage_percent",
    },
    PredefinedQuery {
        category: "System",
        name: "Load average (1m)",
        query: "system_load_average{period=\"1m\"}",
    },

    // Cost
    PredefinedQuery {
        category: "Cost",
        name: "Total session cost (USD)",
        query: "sum(usage_session_cost_usd_total)",
    },
    PredefinedQuery {
        category: "Cost",
        name: "Cost rate (5m)",
        query: "increase(usage_session_cost_usd_total[5m])",
    },
];

/// Returns indices into `PREDEFINED_QUERIES` that match `filter` (case-insensitive
/// substring of name + query). Empty filter returns all indices in original order.
pub fn filtered_indices(filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..PREDEFINED_QUERIES.len()).collect();
    }
    let needle = filter.to_lowercase();
    PREDEFINED_QUERIES
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            q.name.to_lowercase().contains(&needle)
                || q.query.to_lowercase().contains(&needle)
                || q.category.to_lowercase().contains(&needle)
        })
        .map(|(i, _)| i)
        .collect()
}
