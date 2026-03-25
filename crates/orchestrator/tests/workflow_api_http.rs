//! Integration tests for the workflow API HTTP endpoints.
//!
//! Tests the `POST /workflows` handler end-to-end using an in-process Axum
//! router. No real TCP connections are made.
//!
//! # Design
//!
//! HTTP tests use `tower::ServiceExt::oneshot` directly on a cloned Router.
//! This avoids real TCP and is faster than spawning a server.
//!
//! A `NullBackend` implements `ExecutionBackend` with no-op responses so that
//! `AgentManager` can be constructed without a real tmux/Docker environment.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use orchestrator::{
    manager::AgentManager,
    scheduler::{
        api::{workflow_routes, WorkflowState},
        storage::SchedulerStorage,
        Scheduler,
    },
    storage::AgentStorage,
    websocket::ConnectionRegistry,
};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;
use wrap::{
    backend::{SessionConfig, SessionExitInfo, SessionHealth},
    ExecutionBackend,
};

struct NullBackend;

#[async_trait]
impl ExecutionBackend for NullBackend {
    async fn create_session(&self, _config: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn launch_agent(&self, _config: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn session_exists(&self, _session_name: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn kill_session(&self, _session_name: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn send_command(&self, _session_name: &str, _command: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
    fn prefix(&self) -> &str {
        "test"
    }
    async fn session_health(&self, _session_name: &str) -> anyhow::Result<SessionHealth> {
        Ok(SessionHealth::Unknown)
    }
    async fn session_exit_info(
        &self,
        _session_name: &str,
    ) -> anyhow::Result<Option<SessionExitInfo>> {
        Ok(None)
    }
}

async fn build_workflow_app() -> (axum::Router, Arc<Scheduler>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let agent_storage = AgentStorage::with_path(&db_path).await.unwrap();
    let scheduler_storage = SchedulerStorage::new(agent_storage.db().clone());
    let registry = ConnectionRegistry::new();

    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
    let manager = Arc::new(AgentManager::new(
        Arc::new(agent_storage),
        Arc::new(NullBackend),
        registry,
        "ws://localhost:8080".to_string(),
    ));

    let state = WorkflowState { scheduler: scheduler.clone(), manager };
    let router = workflow_routes(state);

    (router, scheduler, temp_dir)
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// Linear trigger filter validation tests
// ---------------------------------------------------------------------------

/// POST /workflows with a `linear_issues` trigger but no filter fields set
/// must be rejected with 400 before reaching the agent-exists check.
#[tokio::test]
async fn test_create_linear_workflow_no_filters_returns_400() {
    let (app, _scheduler, _tmp) = build_workflow_app().await;

    let body = serde_json::json!({
        "name": "linear-no-filters",
        "agent_id": Uuid::new_v4(),
        "trigger_config": {
            "type": "linear_issues"
            // all filter fields intentionally omitted → default to None/empty
        },
        "prompt_template": "Handle issue: {{title}}"
    })
    .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    let error_msg = json["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("at least one filter"),
        "expected filter validation error, got: {error_msg}"
    );
}

/// POST /workflows with `linear_issues` and `team_key` set passes the filter
/// validation. The request proceeds until it fails at the Linear API key check
/// (since no key is configured in the test environment), confirming that the
/// filter gate did not block it.
#[tokio::test]
async fn test_create_linear_workflow_with_team_key_passes_filter_validation() {
    let (app, _scheduler, _tmp) = build_workflow_app().await;

    let body = serde_json::json!({
        "name": "linear-team-key",
        "agent_id": Uuid::new_v4(),
        "trigger_config": {
            "type": "linear_issues",
            "team_key": "ENG"
            // all other filters omitted — team_key alone should satisfy the check
        },
        "prompt_template": "Handle issue: {{title}}"
    })
    .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    // The filter validation must have passed. The response may still be an
    // error (e.g. missing Linear API key or agent not found), but it must NOT
    // be the "at least one filter" error.
    let status = response.status();
    let json = body_json(response).await;
    let error_msg = json["error"].as_str().unwrap_or("");
    assert!(
        !error_msg.contains("at least one filter"),
        "filter validation should have passed for team_key=ENG, but got: {error_msg}"
    );
    // Also confirm we did not get a 201 Created without an API key; we expect
    // some error (400 for API key or agent) rather than a success response.
    assert_ne!(
        status,
        StatusCode::CREATED,
        "should not create workflow without a valid Linear API key or running agent"
    );
}

/// POST /workflows with `linear_issues` and only `labels` set also passes
/// the filter validation (labels is the only Vec<String> field).
#[tokio::test]
async fn test_create_linear_workflow_with_labels_passes_filter_validation() {
    let (app, _scheduler, _tmp) = build_workflow_app().await;

    let body = serde_json::json!({
        "name": "linear-labels",
        "agent_id": Uuid::new_v4(),
        "trigger_config": {
            "type": "linear_issues",
            "labels": ["bug"]
        },
        "prompt_template": "Fix: {{title}}"
    })
    .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/workflows")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    let error_msg = json["error"].as_str().unwrap_or("");
    assert!(
        !error_msg.contains("at least one filter"),
        "filter validation should have passed for labels=[\"bug\"], but got: {error_msg}"
    );
}
