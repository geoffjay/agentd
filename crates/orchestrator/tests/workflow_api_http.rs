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
use chrono::Utc;
use orchestrator::{
    manager::AgentManager,
    scheduler::{
        api::{workflow_routes, WorkflowState},
        storage::SchedulerStorage,
        types::{TriggerConfig, WorkflowConfig},
        Scheduler,
    },
    storage::AgentStorage,
    types::{Agent, AgentConfig, AgentStatus},
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
    let (router, scheduler, _storage, temp_dir) = build_workflow_app_with_storage().await;
    (router, scheduler, temp_dir)
}

/// Like [`build_workflow_app`] but also returns the agent storage so tests
/// can insert agents directly (e.g. to satisfy the agent-running check).
async fn build_workflow_app_with_storage(
) -> (axum::Router, Arc<Scheduler>, Arc<AgentStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let agent_storage = Arc::new(AgentStorage::with_path(&db_path).await.unwrap());
    let scheduler_storage = SchedulerStorage::new(agent_storage.db().clone());
    let registry = ConnectionRegistry::new();

    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
    let manager = Arc::new(AgentManager::new(
        agent_storage.clone(),
        Arc::new(NullBackend),
        registry,
        "ws://localhost:8080".to_string(),
    ));

    let state = WorkflowState { scheduler: scheduler.clone(), manager };
    let router = workflow_routes(state);

    (router, scheduler, agent_storage, temp_dir)
}

/// Insert an agent with the given status directly into storage.
async fn insert_agent(storage: &AgentStorage, name: &str, status: AgentStatus) -> Agent {
    let config: AgentConfig =
        serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
    let mut agent = Agent::new(name.to_string(), config);
    agent.status = status;
    storage.add(&agent).await.unwrap();
    agent
}

/// Insert a workflow with a manual trigger directly into storage.
async fn insert_workflow(
    scheduler: &Scheduler,
    name: &str,
    agent_id: Uuid,
    trigger_config: TriggerConfig,
    enabled: bool,
) -> WorkflowConfig {
    let now = Utc::now();
    let config = WorkflowConfig {
        id: Uuid::new_v4(),
        name: name.to_string(),
        agent_id,
        trigger_config,
        prompt_template: "Task: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
        project_id: None,
        organization_id: None,
    };
    scheduler.storage().add_workflow(&config).await.unwrap();
    config
}

/// Send a PUT /workflows/{id} request with the given JSON body.
async fn put_workflow(
    app: axum::Router,
    id: Uuid,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri(format!("/workflows/{id}"))
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
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

// ---------------------------------------------------------------------------
// PUT /workflows/{id} — trigger_config and agent_id updates
// ---------------------------------------------------------------------------

/// Updating the trigger config of a disabled workflow persists the new
/// trigger without touching any runner.
#[tokio::test]
async fn test_update_trigger_config_on_disabled_workflow_persists() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf = insert_workflow(
        &scheduler,
        "wf-disabled",
        Uuid::new_v4(),
        TriggerConfig::Cron { expression: "0 9 * * *".to_string() },
        false,
    )
    .await;

    let response =
        put_workflow(app, wf.id, serde_json::json!({ "trigger_config": { "type": "manual" } }))
            .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["trigger_config"]["type"], "manual");

    // Persisted, not just echoed.
    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert!(matches!(stored.trigger_config, TriggerConfig::Manual {}));
}

/// An invalid trigger config is rejected with 400 and the stored workflow
/// is left unchanged.
#[tokio::test]
async fn test_update_with_invalid_cron_returns_400_and_preserves_original() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf =
        insert_workflow(&scheduler, "wf-bad-cron", Uuid::new_v4(), TriggerConfig::Manual {}, false)
            .await;

    let response = put_workflow(
        app,
        wf.id,
        serde_json::json!({ "trigger_config": { "type": "cron", "expression": "not a cron" } }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert!(
        matches!(stored.trigger_config, TriggerConfig::Manual {}),
        "original trigger must be preserved on validation failure"
    );
}

/// Changing the trigger config of an enabled workflow restarts its runner
/// so the new trigger takes effect immediately.
#[tokio::test]
async fn test_update_trigger_on_enabled_workflow_restarts_runner() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf =
        insert_workflow(&scheduler, "wf-enabled", Uuid::new_v4(), TriggerConfig::Manual {}, true)
            .await;
    // Simulate the runner being live (as it would be after create/resume).
    scheduler.start_workflow(wf.clone()).await.unwrap();
    assert!(scheduler.running_workflows().await.iter().any(|(id, _)| *id == wf.id));

    let response = put_workflow(
        app,
        wf.id,
        serde_json::json!({ "trigger_config": { "type": "cron", "expression": "0 9 * * *" } }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert!(matches!(stored.trigger_config, TriggerConfig::Cron { .. }));
    assert!(
        scheduler.running_workflows().await.iter().any(|(id, _)| *id == wf.id),
        "runner should be live again after the update restarted it"
    );

    scheduler.shutdown_all().await;
}

/// Re-assigning a workflow to a non-existent agent is rejected with 400.
#[tokio::test]
async fn test_update_agent_id_to_missing_agent_returns_400() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf =
        insert_workflow(&scheduler, "wf-agent", Uuid::new_v4(), TriggerConfig::Manual {}, false)
            .await;

    let response =
        put_workflow(app, wf.id, serde_json::json!({ "agent_id": Uuid::new_v4() })).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert!(json["error"].as_str().unwrap_or("").contains("Agent not found"));
}

/// Re-assigning a workflow to a stopped agent is rejected with 400
/// (parity with create).
#[tokio::test]
async fn test_update_agent_id_to_stopped_agent_returns_400() {
    let (app, scheduler, storage, _tmp) = build_workflow_app_with_storage().await;

    let stopped = insert_agent(&storage, "stopped-agent", AgentStatus::Stopped).await;
    let wf =
        insert_workflow(&scheduler, "wf-stopped", Uuid::new_v4(), TriggerConfig::Manual {}, false)
            .await;

    let response = put_workflow(app, wf.id, serde_json::json!({ "agent_id": stopped.id })).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_json(response).await;
    assert!(json["error"].as_str().unwrap_or("").contains("not running"));
}

/// Re-assigning a workflow to a running agent succeeds and is persisted.
#[tokio::test]
async fn test_update_agent_id_to_running_agent_succeeds() {
    let (app, scheduler, storage, _tmp) = build_workflow_app_with_storage().await;

    let running = insert_agent(&storage, "running-agent", AgentStatus::Running).await;
    let wf =
        insert_workflow(&scheduler, "wf-running", Uuid::new_v4(), TriggerConfig::Manual {}, false)
            .await;

    let response = put_workflow(app, wf.id, serde_json::json!({ "agent_id": running.id })).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["agent_id"], running.id.to_string());

    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert_eq!(stored.agent_id, running.id);
}

/// The legacy `source_config` body key is accepted as an alias for
/// `trigger_config`.
#[tokio::test]
async fn test_update_accepts_source_config_alias() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf = insert_workflow(
        &scheduler,
        "wf-alias",
        Uuid::new_v4(),
        TriggerConfig::Cron { expression: "0 9 * * *".to_string() },
        false,
    )
    .await;

    let response =
        put_workflow(app, wf.id, serde_json::json!({ "source_config": { "type": "manual" } }))
            .await;

    assert_eq!(response.status(), StatusCode::OK);
    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert!(matches!(stored.trigger_config, TriggerConfig::Manual {}));
}

/// Regression: a plain `{"enabled": false}` update still works and stops
/// being reported as enabled.
#[tokio::test]
async fn test_update_enabled_false_still_works() {
    let (app, scheduler, _storage, _tmp) = build_workflow_app_with_storage().await;

    let wf =
        insert_workflow(&scheduler, "wf-toggle", Uuid::new_v4(), TriggerConfig::Manual {}, true)
            .await;

    let response = put_workflow(app, wf.id, serde_json::json!({ "enabled": false })).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["enabled"], false);

    let stored = scheduler.storage().get_workflow(&wf.id).await.unwrap().unwrap();
    assert!(!stored.enabled);
}
