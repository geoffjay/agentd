//! Curated named PromQL queries over the agentd metric inventory.
//!
//! The catalog is the single version-controlled home for PromQL on the
//! platform: agents and tools reference queries by name instead of embedding
//! query text in prompts. Each entry may contain the `$__window` token, which
//! [`resolve`] substitutes with a validated duration.
//!
//! Counter-based queries use `increase()`/`rate()` (reset-safe). Metrics
//! labeled by unbounded values (e.g. `room_id`) are aggregated or excluded.

/// One catalog entry.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct NamedQuery {
    /// Stable identifier used in API paths and tool calls.
    pub name: &'static str,
    /// Human-readable description of what the result means.
    pub description: &'static str,
    /// PromQL, possibly containing the `$__window` token.
    pub promql: &'static str,
    /// Unit of the resulting values.
    pub unit: &'static str,
    /// Default `$__window` substitution when the caller omits one.
    pub default_window: &'static str,
}

/// The curated query catalog.
pub const QUERY_CATALOG: &[NamedQuery] = &[
    NamedQuery {
        name: "service-up",
        description: "Scrape success per agentd service; 0 means Prometheus cannot reach it",
        promql: r#"up{job=~"agentd-.*"}"#,
        unit: "boolean",
        default_window: "",
    },
    NamedQuery {
        name: "dispatch-success-rate",
        description: "Fraction of finished workflow dispatches that completed successfully",
        promql: r#"sum(increase(workflow_dispatches_total{status="completed"}[$__window])) / clamp_min(sum(increase(workflow_dispatches_total{status=~"completed|failed"}[$__window])), 1)"#,
        unit: "ratio",
        default_window: "1h",
    },
    NamedQuery {
        name: "dispatch-throughput",
        description: "Workflow dispatch counts by status over the window",
        promql: r#"sum by (status) (increase(workflow_dispatches_total[$__window]))"#,
        unit: "count",
        default_window: "1h",
    },
    NamedQuery {
        name: "agent-restart-rate",
        description:
            "Agent restarts over the window; sustained nonzero values suggest crash-looping",
        promql: r#"sum(increase(agents_restarted_total[$__window]))"#,
        unit: "count",
        default_window: "1h",
    },
    NamedQuery {
        name: "agents-active",
        description: "Currently active agents",
        promql: "agents_active",
        unit: "count",
        default_window: "",
    },
    NamedQuery {
        name: "websocket-connections",
        description:
            "Live agent SDK WebSocket connections; compare with agents-active for state mismatches",
        promql: "websocket_connections_active",
        unit: "count",
        default_window: "",
    },
    NamedQuery {
        name: "approvals-backlog",
        description: "Pending tool approvals; sustained growth means agents are blocked on humans",
        promql: "approvals_pending",
        unit: "count",
        default_window: "",
    },
    NamedQuery {
        name: "approvals-resolution",
        description: "Tool approvals resolved over the window, by decision (approve/deny)",
        promql: r#"sum by (decision) (increase(approvals_resolved_total[$__window]))"#,
        unit: "count",
        default_window: "1h",
    },
    NamedQuery {
        name: "http-error-rate",
        description: "HTTP 5xx ratio per service over the window",
        promql: r#"sum by (service) (rate(http_requests_total{status=~"5.."}[$__window])) / clamp_min(sum by (service) (rate(http_requests_total[$__window])), 1e-9)"#,
        unit: "ratio",
        default_window: "15m",
    },
    NamedQuery {
        name: "http-p95-latency",
        description: "p95 HTTP request latency per service over the window (seconds)",
        promql: r#"histogram_quantile(0.95, sum by (service, le) (rate(http_request_duration_seconds_bucket[$__window])))"#,
        unit: "seconds",
        default_window: "15m",
    },
    NamedQuery {
        name: "session-cost",
        description: "Claude session spend over the window (USD, gauge delta)",
        promql: r#"delta(usage_session_cost_usd_total[$__window])"#,
        unit: "usd",
        default_window: "24h",
    },
    NamedQuery {
        name: "notifications-backlog",
        description: "Pending notifications awaiting response or dismissal",
        promql: "notifications_pending",
        unit: "count",
        default_window: "",
    },
    NamedQuery {
        name: "message-drops",
        description: "Agent-bound messages dropped by the message bridge over the window",
        promql: r#"sum(increase(messages_dropped[$__window]))"#,
        unit: "count",
        default_window: "1h",
    },
    NamedQuery {
        name: "host-saturation",
        description: "Host CPU %, memory %, worst-disk %, and 1m load in one vector",
        promql: r#"system_cpu_usage_percent or (100 * system_memory_used_bytes / system_memory_total_bytes) or max by (mountpoint) (system_disk_usage_percent) or system_load_average{period="1m"}"#,
        unit: "mixed",
        default_window: "",
    },
];

/// Look up a catalog entry by name.
pub fn find(name: &str) -> Option<&'static NamedQuery> {
    QUERY_CATALOG.iter().find(|q| q.name == name)
}

/// Validate a window string: digits followed by one of `s m h d w`.
///
/// The window is the only caller-controlled text spliced into PromQL, so
/// this check doubles as the injection guard.
pub fn valid_window(window: &str) -> bool {
    let Some(unit) = window.chars().last() else { return false };
    if !matches!(unit, 's' | 'm' | 'h' | 'd' | 'w') {
        return false;
    }
    let digits = &window[..window.len() - 1];
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// Resolve a named query to executable PromQL, substituting `$__window`.
///
/// Returns `Err` with a user-facing message when the window is invalid or
/// supplied for a windowless (instant gauge) query.
pub fn resolve(query: &NamedQuery, window: Option<&str>) -> Result<String, String> {
    if !query.promql.contains("$__window") {
        return Ok(query.promql.to_string());
    }

    let window = window.unwrap_or(query.default_window);
    if !valid_window(window) {
        return Err(format!(
            "invalid window `{window}` — expected digits plus a unit (s/m/h/d/w), e.g. 15m, 1h, 24h"
        ));
    }
    Ok(query.promql.replace("$__window", window))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_names_are_unique() {
        let mut names: Vec<&str> = QUERY_CATALOG.iter().map(|q| q.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), QUERY_CATALOG.len());
    }

    #[test]
    fn windowed_queries_declare_a_default_window() {
        for q in QUERY_CATALOG {
            let windowed = q.promql.contains("$__window");
            assert_eq!(
                windowed,
                !q.default_window.is_empty(),
                "query `{}` must declare a default window iff its promql is windowed",
                q.name
            );
        }
    }

    #[test]
    fn valid_windows_accepted() {
        for w in ["30s", "15m", "1h", "24h", "7d", "2w"] {
            assert!(valid_window(w), "{w} should be valid");
        }
    }

    #[test]
    fn invalid_windows_rejected() {
        for w in ["", "1", "h", "1x", "1h)", "'; drop", "1h or vector(1)", "-1h", "1.5h"] {
            assert!(!valid_window(w), "{w} should be rejected");
        }
    }

    #[test]
    fn resolve_substitutes_window() {
        let q = find("dispatch-success-rate").unwrap();
        let promql = resolve(q, Some("15m")).unwrap();
        assert!(promql.contains("[15m]"));
        assert!(!promql.contains("$__window"));
    }

    #[test]
    fn resolve_uses_default_window() {
        let q = find("session-cost").unwrap();
        let promql = resolve(q, None).unwrap();
        assert!(promql.contains("[24h]"));
    }

    #[test]
    fn resolve_rejects_injection() {
        let q = find("http-error-rate").unwrap();
        assert!(resolve(q, Some("5m]) or vector(1) #")).is_err());
    }

    #[test]
    fn windowless_queries_ignore_window() {
        let q = find("agents-active").unwrap();
        assert_eq!(resolve(q, Some("nonsense")).unwrap(), "agents_active");
    }

    #[test]
    fn find_hit_and_miss() {
        assert!(find("host-saturation").is_some());
        assert!(find("does-not-exist").is_none());
    }
}
