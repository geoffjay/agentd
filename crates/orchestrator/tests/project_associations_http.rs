//! Integration tests for project association endpoints.
//!
//! After project CRUD was moved to the core service (#1313), the orchestrator
//! retains four association endpoints:
//!
//! - `POST   /projects/{id}/agents/{agent_id}`    — associate agent with project
//! - `DELETE /projects/{id}/agents/{agent_id}`    — dissociate agent from project
//! - `POST   /projects/{id}/workflows/{wf_id}`   — associate workflow with project
//! - `DELETE /projects/{id}/workflows/{wf_id}`   — dissociate workflow from project
//!
//! The orchestrator verifies that the agent / workflow exists (returns 404 if
//! not) but does **not** cross-check with the core service to verify the
//! project ID — that validation is the caller's responsibility.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use communicate::client::CommunicateClient;
use orchestrator::{
    api::{create_router, ApiState},
    manager::AgentManager,
    scheduler::{
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
    types::BackendType,
    ExecutionBackend,
};

// ---------------------------------------------------------------------------
// No-op backend
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Test app builder
// ---------------------------------------------------------------------------

async fn build_app() -> (axum::Router, Arc<AgentStorage>, Arc<Scheduler>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let storage = Arc::new(AgentStorage::with_path(&db_path).await.unwrap());
    let scheduler_storage = SchedulerStorage::new(storage.db().clone());
    let registry = ConnectionRegistry::new();
    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
    let manager = Arc::new(
        AgentManager::new(
            storage.clone(),
            Arc::new(NullBackend),
            registry.clone(),
            "ws://localhost:7006".to_string(),
        )
        .with_mcp_config_dir(temp_dir.path().join("mcp")),
    );
    let communicate = CommunicateClient::new("http://localhost:17010");

    let state = ApiState {
        manager,
        registry,
        scheduler: scheduler.clone(),
        communicate,
        backend_type: BackendType::Tmux,
    };

    (create_router(state), storage, scheduler, temp_dir)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Insert an agent with the given status directly into storage.
async fn insert_agent(storage: &AgentStorage, name: &str) -> Agent {
    let config: AgentConfig =
        serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
    let mut agent = Agent::new(name.to_string(), config);
    agent.status = AgentStatus::Pending;
    storage.add(&agent).await.unwrap();
    agent
}

/// Insert a workflow with a manual trigger directly into scheduler storage.
async fn insert_workflow(scheduler: &Scheduler, name: &str, agent_id: Uuid) -> WorkflowConfig {
    let now = Utc::now();
    let config = WorkflowConfig {
        id: Uuid::new_v4(),
        name: name.to_string(),
        agent_id,
        trigger_config: TriggerConfig::Manual {},
        prompt_template: "Task: {{title}}".to_string(),
        poll_interval_secs: 60,
        enabled: true,
        tool_policy: Default::default(),
        created_at: now,
        updated_at: now,
        project_id: None,
        organization_id: None,
    };
    scheduler.storage().add_workflow(&config).await.unwrap();
    config
}

// ---------------------------------------------------------------------------
// Agent association tests
// ---------------------------------------------------------------------------

/// `POST /projects/{project_id}/agents/{agent_id}` with a valid agent returns 204.
#[tokio::test]
async fn test_associate_agent_with_project_returns_204() {
    let (app, storage, _scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-assoc-agent-1").await;
    let project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/agents/{}", agent.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// `DELETE /projects/{project_id}/agents/{agent_id}` with a valid agent returns 204.
#[tokio::test]
async fn test_dissociate_agent_from_project_returns_204() {
    let (app, storage, _scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-assoc-agent-2").await;
    let project_id = Uuid::new_v4();

    // Associate first, then dissociate.
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/agents/{}", agent.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{project_id}/agents/{}", agent.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// `POST /projects/{project_id}/agents/{agent_id}` with an unknown agent_id returns 404.
#[tokio::test]
async fn test_associate_agent_nonexistent_agent_returns_404() {
    let (app, _storage, _scheduler, _tmp) = build_app().await;
    let project_id = Uuid::new_v4();
    let missing_agent_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/agents/{missing_agent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Associating an agent with a non-existent project UUID still returns 204
/// because the orchestrator does not validate project IDs against the core
/// service — that check is the caller's responsibility.
#[tokio::test]
async fn test_associate_agent_nonexistent_project_still_succeeds() {
    let (app, storage, _scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-assoc-agent-3").await;
    let nonexistent_project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{nonexistent_project_id}/agents/{}", agent.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 204 — orchestrator only validates that the agent exists, not the project.
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// Workflow association tests
// ---------------------------------------------------------------------------

/// `POST /projects/{project_id}/workflows/{wf_id}` with a valid workflow returns 204.
#[tokio::test]
async fn test_associate_workflow_with_project_returns_204() {
    let (app, storage, scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-wf-assoc-agent-1").await;
    let workflow = insert_workflow(&scheduler, "orch-wf-1", agent.id).await;
    let project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/workflows/{}", workflow.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// `DELETE /projects/{project_id}/workflows/{wf_id}` with a valid workflow returns 204.
#[tokio::test]
async fn test_dissociate_workflow_from_project_returns_204() {
    let (app, storage, scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-wf-assoc-agent-2").await;
    let workflow = insert_workflow(&scheduler, "orch-wf-2", agent.id).await;
    let project_id = Uuid::new_v4();

    // Associate first.
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/workflows/{}", workflow.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/projects/{project_id}/workflows/{}", workflow.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// `POST /projects/{project_id}/workflows/{wf_id}` with an unknown workflow returns 404.
#[tokio::test]
async fn test_associate_workflow_nonexistent_workflow_returns_404() {
    let (app, _storage, _scheduler, _tmp) = build_app().await;
    let project_id = Uuid::new_v4();
    let missing_wf_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/workflows/{missing_wf_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// List association tests
// ---------------------------------------------------------------------------

/// After associating an agent, `GET /projects/{id}/agents` returns that agent.
#[tokio::test]
async fn test_list_project_agents_returns_associated_agents() {
    let (app, storage, _scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-list-agent").await;
    let project_id = Uuid::new_v4();

    // Associate the agent.
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/agents/{}", agent.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{project_id}/agents"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], agent.id.to_string());
}

/// After associating a workflow, `GET /projects/{id}/workflows` returns that workflow.
#[tokio::test]
async fn test_list_project_workflows_returns_associated_workflows() {
    let (app, storage, scheduler, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "orch-list-wf-agent").await;
    let workflow = insert_workflow(&scheduler, "orch-list-wf", agent.id).await;
    let project_id = Uuid::new_v4();

    // Associate the workflow.
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/projects/{project_id}/workflows/{}", workflow.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/projects/{project_id}/workflows"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], workflow.id.to_string());
}
