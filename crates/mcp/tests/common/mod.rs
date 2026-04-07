//! Shared test infrastructure for agentd-mcp integration tests.
//!
//! Provides lightweight mock servers built with axum that simulate agentd
//! service APIs. Tests bind on a random OS-assigned port so multiple test
//! processes can run concurrently without port conflicts.

#![allow(dead_code)]

use agentd_mcp::{client::AgentdClient, config::AgentdMcpConfig};
use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

// ---------------------------------------------------------------------------
// Fixture data
// ---------------------------------------------------------------------------

pub fn mock_agent_running() -> Value {
    json!({
        "id": "aaaaaaaa-0000-0000-0000-000000000001",
        "name": "test-agent-running",
        "status": "running",
        "activity": "idle",
        "config": {
            "working_dir": "/tmp/agent",
            "model": "claude-sonnet-4-5",
            "tool_policy": { "mode": "allow_all" },
            "interactive": false,
            "worktree": false,
            "env": {}
        },
        "session_id": "test-agent-running",
        "backend_type": "tmux",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    })
}

pub fn mock_agent_failed() -> Value {
    json!({
        "id": "bbbbbbbb-0000-0000-0000-000000000002",
        "name": "test-agent-failed",
        "status": "failed",
        "activity": "idle",
        "config": {
            "working_dir": "/tmp/failed",
            "model": "claude-sonnet-4-5",
            "tool_policy": { "mode": "allow_all" },
            "interactive": false,
            "worktree": false,
            "env": {}
        },
        "session_id": null,
        "backend_type": "tmux",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T01:00:00Z"
    })
}

pub fn mock_workflow() -> Value {
    json!({
        "id": "cccccccc-0000-0000-0000-000000000003",
        "name": "test-workflow",
        "agent_id": "aaaaaaaa-0000-0000-0000-000000000001",
        "trigger_config": {
            "type": "github_issues",
            "owner": "testorg",
            "repo": "testrepo",
            "labels": ["bug"],
            "state": "open"
        },
        "prompt_template": "Fix issue: {{title}}\n\n{{body}}",
        "poll_interval_secs": 60,
        "enabled": true,
        "tool_policy": { "mode": "allow_all" },
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    })
}

pub fn mock_notification() -> Value {
    json!({
        "id": "dddddddd-0000-0000-0000-000000000004",
        "source": { "type": "system" },
        "lifetime": { "type": "persistent" },
        "priority": "high",
        "status": "pending",
        "title": "Test Notification",
        "message": "This is a test notification for integration testing.",
        "requires_response": false,
        "response": null,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    })
}

pub fn mock_approval() -> Value {
    json!({
        "id": "eeeeeeee-0000-0000-0000-000000000005",
        "agent_id": "aaaaaaaa-0000-0000-0000-000000000001",
        "tool_name": "Read",
        "tool_input": { "file_path": "/tmp/test.txt" },
        "status": "pending",
        "created_at": "2026-01-01T00:00:00Z",
        "expires_at": "2026-01-01T01:00:00Z"
    })
}

// ---------------------------------------------------------------------------
// Mock server handle
// ---------------------------------------------------------------------------

/// A running mock server that can be stopped by dropping the handle.
pub struct MockServer {
    pub addr: SocketAddr,
    _handle: tokio::task::JoinHandle<()>,
}

impl MockServer {
    /// Start a new mock server with the given router.
    pub async fn start(router: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { addr, _handle: handle }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

// ---------------------------------------------------------------------------
// Mock orchestrator
// ---------------------------------------------------------------------------

pub async fn mock_orchestrator_server() -> MockServer {
    let router = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok", "version": "0.1.0"})) }))
        .route("/agents", get(list_agents_handler).post(create_agent_handler))
        .route(
            "/agents/{id}",
            get(get_agent_handler).delete(|| async { (axum::http::StatusCode::NO_CONTENT, "") }),
        )
        .route("/agents/{id}/message", post(|| async { Json(json!({"status": "sent"})) }))
        .route(
            "/agents/{id}/approvals",
            get(|Path(id): Path<String>| async move {
                if id == "aaaaaaaa-0000-0000-0000-000000000001" {
                    Json(vec![mock_approval()])
                } else {
                    Json(vec![])
                }
            }),
        )
        .route(
            "/agents/{id}/usage",
            get(|| async {
                Json(json!({
                    "total_input_tokens": 1000,
                    "total_output_tokens": 500,
                    "session_count": 3
                }))
            }),
        )
        .route("/approvals", get(|| async { Json(vec![mock_approval()]) }))
        .route("/approvals/{id}/approve", post(|| async { Json(json!({"status": "approved"})) }))
        .route("/approvals/{id}/deny", post(|| async { Json(json!({"status": "denied"})) }))
        .route(
            "/workflows",
            get(|| async {
                Json(json!({
                    "items": [mock_workflow()],
                    "total": 1,
                    "limit": 100,
                    "offset": 0
                }))
            }),
        )
        .route("/workflows/{id}", get(get_workflow_handler))
        .route("/workflows/{id}/history", get(dispatch_history_handler))
        .route(
            "/metrics",
            get(|| async {
                "# HELP orchestrator_agents_total Total agents\n\
             # TYPE orchestrator_agents_total gauge\n\
             orchestrator_agents_total 2\n\
             orchestrator_agents_running 1\n"
            }),
        );

    MockServer::start(router).await
}

async fn list_agents_handler(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let all = vec![mock_agent_running(), mock_agent_failed()];
    let items: Vec<Value> = if let Some(status) = params.get("status") {
        all.into_iter().filter(|a| a["status"].as_str() == Some(status.as_str())).collect()
    } else {
        all
    };
    let total = items.len() as u64;
    Json(json!({ "items": items, "total": total, "limit": 100, "offset": 0 }))
}

async fn get_agent_handler(Path(id): Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if id == "aaaaaaaa-0000-0000-0000-000000000001" {
        Json(mock_agent_running()).into_response()
    } else if id == "bbbbbbbb-0000-0000-0000-000000000002" {
        Json(mock_agent_failed()).into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
    }
}

async fn create_agent_handler() -> (axum::http::StatusCode, Json<Value>) {
    (
        axum::http::StatusCode::CREATED,
        Json(json!({
            "id": "11111111-0000-0000-0000-000000000099",
            "name": "new-agent",
            "status": "pending"
        })),
    )
}

async fn get_workflow_handler(Path(id): Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if id == "cccccccc-0000-0000-0000-000000000003" {
        Json(mock_workflow()).into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
    }
}

async fn dispatch_history_handler(Path(id): Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if id == "cccccccc-0000-0000-0000-000000000003" {
        Json(json!({
            "items": [{
                "id": "ffffffff-0000-0000-0000-000000000006",
                "workflow_id": "cccccccc-0000-0000-0000-000000000003",
                "source_id": "42",
                "agent_id": "aaaaaaaa-0000-0000-0000-000000000001",
                "prompt_sent": "Fix issue #42: Test bug\n\nSome details here.",
                "status": "completed",
                "dispatched_at": "2026-01-01T00:00:00Z",
                "completed_at": "2026-01-01T00:05:00Z"
            }],
            "total": 1,
            "limit": 200,
            "offset": 0
        }))
        .into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
    }
}

// ---------------------------------------------------------------------------
// Mock notify server
// ---------------------------------------------------------------------------

pub async fn mock_notify_server() -> MockServer {
    let router = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok", "version": "0.1.0"})) }))
        .route("/notifications", get(list_notifications_handler).post(create_notification_handler))
        .route(
            "/notifications/actionable",
            get(|| async {
                Json(json!({
                    "items": [mock_notification()],
                    "total": 1,
                    "limit": 50,
                    "offset": 0
                }))
            }),
        )
        .route(
            "/notifications/{id}",
            get(get_notification_handler).delete(dismiss_notification_handler),
        );

    MockServer::start(router).await
}

async fn list_notifications_handler(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let items = if params.get("status").map(|s| s == "pending").unwrap_or(true) {
        vec![mock_notification()]
    } else {
        vec![]
    };
    let total = items.len() as u64;
    Json(json!({ "items": items, "total": total, "limit": 20, "offset": 0 }))
}

async fn get_notification_handler(Path(id): Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if id == "dddddddd-0000-0000-0000-000000000004" {
        Json(mock_notification()).into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
    }
}

async fn create_notification_handler() -> (axum::http::StatusCode, Json<Value>) {
    (axum::http::StatusCode::CREATED, Json(mock_notification()))
}

async fn dismiss_notification_handler(Path(id): Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    if id == "dddddddd-0000-0000-0000-000000000004" {
        axum::http::StatusCode::NO_CONTENT.into_response()
    } else {
        (axum::http::StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response()
    }
}

// ---------------------------------------------------------------------------
// Mock monitor server
// ---------------------------------------------------------------------------

pub async fn mock_monitor_server() -> MockServer {
    let router = Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route(
            "/metrics",
            get(|| async {
                Json(json!({
                    "cpu_usage_percent": 42.5,
                    "memory_used_bytes": 2_147_483_648u64,
                    "memory_total_bytes": 8_589_934_592u64,
                    "disk_used_bytes": 107_374_182_400u64,
                    "disk_total_bytes": 536_870_912_000u64,
                    "load_average": [1.2, 0.9, 0.7]
                }))
            }),
        )
        .route("/status", get(|| async { Json(json!({ "alerts": [] })) }));

    MockServer::start(router).await
}

// ---------------------------------------------------------------------------
// Client builder
// ---------------------------------------------------------------------------

/// Build an `AgentdClient` pointed at the given mock servers.
pub fn test_client(orch_addr: &str, notify_addr: &str, monitor_addr: &str) -> AgentdClient {
    let config = Arc::new(AgentdMcpConfig {
        orchestrator_url: orch_addr.to_string(),
        communicate_url: "http://127.0.0.1:1".to_string(), // unused
        memory_url: "http://127.0.0.1:1".to_string(),
        notify_url: notify_addr.to_string(),
        ask_url: "http://127.0.0.1:1".to_string(),
        wrap_url: "http://127.0.0.1:1".to_string(),
        monitor_url: monitor_addr.to_string(),
        hook_url: "http://127.0.0.1:1".to_string(),
    });
    AgentdClient::new(config)
}
