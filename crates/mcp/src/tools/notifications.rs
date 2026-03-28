//! Notification inspection and management tools for the agentd MCP server.
//!
//! Provides tools for listing, creating, and dismissing notifications from the
//! agentd notify service. Useful for surfacing diagnostic findings and managing
//! the notification backlog.

use crate::client::AgentdClient;
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A notification as returned by the notify service.
#[derive(Debug, Deserialize)]
struct Notification {
    id: String,
    source: serde_json::Value,
    priority: String,
    status: String,
    title: String,
    message: String,
    requires_response: bool,
    #[serde(default)]
    response: Option<String>,
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
// Helpers
// ---------------------------------------------------------------------------

fn priority_icon(priority: &str) -> &'static str {
    match priority {
        "urgent" => "🚨",
        "high" => "🔴",
        "normal" => "🟡",
        "low" => "🔵",
        _ => "⚪",
    }
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "pending" => "📬",
        "viewed" => "👁️",
        "responded" => "✅",
        "dismissed" => "🗑️",
        "expired" => "⏰",
        _ => "❓",
    }
}

fn source_label(source: &serde_json::Value) -> String {
    match source["type"].as_str() {
        Some("system") => "System".to_string(),
        Some("agent_hook") => {
            let agent = source["agent_id"].as_str().unwrap_or("?");
            let hook = source["hook_type"].as_str().unwrap_or("?");
            format!("AgentHook({hook}@{agent})")
        }
        Some("ask_service") => {
            let id = source["request_id"].as_str().unwrap_or("?");
            format!("Ask({id})")
        }
        Some("monitor_service") => {
            let alert = source["alert_type"].as_str().unwrap_or("?");
            format!("Monitor({alert})")
        }
        Some(other) => other.to_string(),
        None => "Unknown".to_string(),
    }
}

fn format_notification_row(n: &Notification) -> String {
    let p = priority_icon(&n.priority);
    let s = status_icon(&n.status);
    let source = source_label(&n.source);
    format!("| {p} {} | {s} {} | {} | `{}` | {} |\n", n.priority, n.status, n.title, n.id, source)
}

fn format_notification_detail(n: &Notification) -> String {
    let p = priority_icon(&n.priority);
    let s = status_icon(&n.status);
    let source = source_label(&n.source);
    let requires = if n.requires_response { "Yes" } else { "No" };

    let mut out = format!("## Notification: {}\n\n", n.title);
    out.push_str(&format!("- **ID**: `{}`\n", n.id));
    out.push_str(&format!("- **Priority**: {p} {}\n", n.priority));
    out.push_str(&format!("- **Status**: {s} {}\n", n.status));
    out.push_str(&format!("- **Source**: {source}\n"));
    out.push_str(&format!("- **Requires response**: {requires}\n"));
    out.push_str(&format!("- **Created**: {}\n", n.created_at));
    out.push_str(&format!("- **Updated**: {}\n\n", n.updated_at));
    out.push_str("### Message\n\n");
    out.push_str(&n.message);
    out.push('\n');

    if let Some(resp) = &n.response {
        out.push_str("\n### Response\n\n");
        out.push_str(resp);
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

/// List notifications with optional filters.
pub async fn run_list_notifications(
    client: &AgentdClient,
    status: Option<&str>,
    priority: Option<&str>,
    limit: Option<u32>,
) -> String {
    let base = client.notify_url();
    let limit_val = limit.unwrap_or(20).clamp(1, 200);
    let mut url = format!("{base}/notifications?limit={limit_val}");
    if let Some(s) = status {
        url.push_str(&format!("&status={s}"));
    }

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: notify service returned HTTP {}", resp.status());
    }

    let page: Paginated<Notification> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing notification list: {e}"),
    };

    // Apply priority filter client-side (API only supports status filter)
    let items: Vec<&Notification> =
        page.items.iter().filter(|n| priority.is_none_or(|p| n.priority == p)).collect();

    if items.is_empty() {
        return "No notifications found matching the specified filters.".to_string();
    }

    let filter_note = match (status, priority) {
        (Some(s), Some(p)) => format!(" (status: {s}, priority: {p})"),
        (Some(s), None) => format!(" (status: {s})"),
        (None, Some(p)) => format!(" (priority: {p})"),
        (None, None) => String::new(),
    };

    let mut out = format!("## Notifications{filter_note} — {} total\n\n", page.total);
    out.push_str("| Priority | Status | Title | ID | Source |\n");
    out.push_str("|----------|--------|-------|----|--------|\n");

    for n in &items {
        out.push_str(&format_notification_row(n));
    }

    out
}

/// Get full details of a specific notification.
pub async fn run_get_notification(client: &AgentdClient, notification_id: &str) -> String {
    let base = client.notify_url();
    let url = format!("{base}/notifications/{notification_id}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Error: notification {notification_id} not found");
    }
    if !resp.status().is_success() {
        return format!("Error: notify service returned HTTP {}", resp.status());
    }

    let n: Notification = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing notification: {e}"),
    };

    format_notification_detail(&n)
}

/// Get actionable notifications (pending or viewed, not expired).
pub async fn run_get_actionable_notifications(client: &AgentdClient) -> String {
    let base = client.notify_url();
    let url = format!("{base}/notifications/actionable?limit=50");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        return format!("Error: notify service returned HTTP {}", resp.status());
    }

    let page: Paginated<Notification> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing actionable notifications: {e}"),
    };

    if page.items.is_empty() {
        return "✅ No actionable notifications — backlog is clear.".to_string();
    }

    let mut out = format!("## Actionable Notifications — {} pending\n\n", page.total);
    out.push_str(
        "> These notifications are awaiting a response or review. \
         Use `dismiss_notification` to clear resolved items.\n\n",
    );
    out.push_str("| Priority | Status | Title | ID | Source |\n");
    out.push_str("|----------|--------|-------|----|--------|\n");

    for n in &page.items {
        out.push_str(&format_notification_row(n));
    }

    out
}

/// Create a system notification.
pub async fn run_create_notification(
    client: &AgentdClient,
    title: &str,
    message: &str,
    priority: Option<&str>,
) -> String {
    let base = client.notify_url();
    let url = format!("{base}/notifications");
    let priority_str = priority.unwrap_or("normal");

    let body = json!({
        "source": { "type": "system" },
        "lifetime": { "type": "persistent" },
        "priority": priority_str,
        "title": title,
        "message": message,
        "requires_response": false
    });

    let resp = match client.inner.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return format!("Error: notify service returned HTTP {status}: {body}");
    }

    let n: Notification = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing created notification: {e}"),
    };

    format!(
        "✅ Notification created.\n\n- **ID**: `{}`\n- **Title**: {}\n- **Priority**: {}\n",
        n.id, n.title, n.priority
    )
}

/// Dismiss a notification by ID.
pub async fn run_dismiss_notification(client: &AgentdClient, notification_id: &str) -> String {
    let base = client.notify_url();
    let url = format!("{base}/notifications/{notification_id}");

    let resp = match client.inner.delete(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: notify service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Error: notification {notification_id} not found");
    }
    if !resp.status().is_success() {
        return format!("Error: notify service returned HTTP {}", resp.status());
    }

    format!("✅ Notification `{notification_id}` dismissed.")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_icon() {
        assert_eq!(priority_icon("urgent"), "🚨");
        assert_eq!(priority_icon("high"), "🔴");
        assert_eq!(priority_icon("normal"), "🟡");
        assert_eq!(priority_icon("low"), "🔵");
        assert_eq!(priority_icon("unknown"), "⚪");
    }

    #[test]
    fn test_status_icon() {
        assert_eq!(status_icon("pending"), "📬");
        assert_eq!(status_icon("viewed"), "👁️");
        assert_eq!(status_icon("responded"), "✅");
        assert_eq!(status_icon("dismissed"), "🗑️");
        assert_eq!(status_icon("expired"), "⏰");
    }

    #[test]
    fn test_source_label_system() {
        let src = serde_json::json!({"type": "system"});
        assert_eq!(source_label(&src), "System");
    }

    #[test]
    fn test_source_label_monitor() {
        let src = serde_json::json!({"type": "monitor_service", "alert_type": "high_cpu"});
        assert_eq!(source_label(&src), "Monitor(high_cpu)");
    }

    #[test]
    fn test_source_label_agent_hook() {
        let src = serde_json::json!({
            "type": "agent_hook",
            "agent_id": "abc-123",
            "hook_type": "pre_tool"
        });
        assert_eq!(source_label(&src), "AgentHook(pre_tool@abc-123)");
    }
}
