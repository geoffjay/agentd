//! Integration tests for system (built-in) agent lifecycle and API separation.
//!
//! Tests the full system-agent feature surface:
//! - `GET /system-agents` returns only built-in agents
//! - `GET /agents` excludes built-in agents by default
//! - `GET /agents?include_builtin=true` includes all agents
//! - `DELETE /agents/{id}` is blocked for built-in agents
//! - `POST /agents` ignores `built_in` from the request body
//! - `AgentManager::bootstrap_system_agents()` creates the system agent correctly
//! - Bootstrap is idempotent — no duplicate agents on repeated calls
//!
//! # Design
//!
//! HTTP tests drive the full Axum router via `tower::ServiceExt::oneshot` with
//! no real TCP connection.  A `NullBackend` replaces tmux/Docker so that
//! `AgentManager` can be constructed without external dependencies.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use communicate::client::CommunicateClient;
use orchestrator::{
    api::{create_router, ApiState},
    manager::AgentManager,
    scheduler::{storage::SchedulerStorage, Scheduler},
    storage::AgentStorage,
    system_agents::SYSTEM_AGENT_NAME,
    types::{Agent, AgentConfig, ToolPolicy},
    websocket::ConnectionRegistry,
};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use wrap::{
    backend::{SessionConfig, SessionExitInfo, SessionHealth},
    types::BackendType,
    ExecutionBackend,
};

// ---------------------------------------------------------------------------
// NullBackend — no-op execution backend for tests
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
    async fn session_pid(&self, _session_name: &str) -> anyhow::Result<Option<u32>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the full orchestrator API router with a temp database.
///
/// Returns the router, an `Arc<AgentStorage>` for direct storage manipulation,
/// and the `TempDir` that must be kept alive for the duration of the test.
async fn build_app() -> (axum::Router, Arc<AgentStorage>, Arc<AgentManager>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let storage = Arc::new(AgentStorage::with_path(&db_path).await.unwrap());
    let scheduler_storage = SchedulerStorage::new(storage.db().clone());
    let registry = ConnectionRegistry::new();
    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
    let manager = Arc::new(AgentManager::new(
        storage.clone(),
        Arc::new(NullBackend),
        registry.clone(),
        "ws://localhost:7006".to_string(),
    ));
    let communicate = CommunicateClient::new("http://localhost:17010");

    let state = ApiState {
        manager: manager.clone(),
        registry,
        scheduler,
        communicate,
        backend_type: BackendType::Tmux,
    };

    let router = create_router(state);
    (router, storage, manager, temp_dir)
}

/// Decode the response body as JSON.
async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Insert a built-in agent directly into storage (bypassing the API,
/// which intentionally never sets `built_in = true`).
async fn insert_builtin_agent(storage: &AgentStorage, name: &str) -> Agent {
    use std::collections::HashMap;
    let mut agent = Agent::new(
        name.to_string(),
        AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: ToolPolicy::default(),
            model: Some("sonnet".to_string()),
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec!["system".to_string()],
        },
    );
    agent.built_in = true;
    storage.add(&agent).await.unwrap();
    agent
}

/// Insert a regular user agent into storage.
async fn insert_user_agent(storage: &AgentStorage, name: &str) -> Agent {
    use std::collections::HashMap;
    let agent = Agent::new(
        name.to_string(),
        AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: false,
            system_prompt: None,
            system_prompt_file: None,
            append_system_prompt: false,
            tool_policy: ToolPolicy::default(),
            model: None,
            env: HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec![],
            rooms: vec![],
        },
    );
    storage.add(&agent).await.unwrap();
    agent
}

// ---------------------------------------------------------------------------
// Tests: GET /system-agents
// ---------------------------------------------------------------------------

/// `GET /system-agents` returns only built-in agents.
#[tokio::test]
async fn test_system_agents_endpoint_returns_builtin_only() {
    let (app, storage, _manager, _tmp) = build_app().await;

    let builtin = insert_builtin_agent(&storage, "agentd-system").await;
    insert_user_agent(&storage, "user-agent-1").await;

    let response = app
        .oneshot(Request::builder().uri("/system-agents").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let agents = json.as_array().expect("expected array");

    assert_eq!(agents.len(), 1, "should return exactly 1 system agent");
    assert_eq!(agents[0]["name"], builtin.name);
    assert_eq!(agents[0]["built_in"], true, "built_in should be true");
}

/// `GET /system-agents` returns an empty array when no built-in agents exist.
#[tokio::test]
async fn test_system_agents_endpoint_empty_when_no_builtin() {
    let (app, storage, _manager, _tmp) = build_app().await;
    insert_user_agent(&storage, "user-only").await;

    let response = app
        .oneshot(Request::builder().uri("/system-agents").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let agents = json.as_array().expect("expected array");
    assert!(agents.is_empty(), "should return empty array with no built-in agents");
}

// ---------------------------------------------------------------------------
// Tests: GET /agents (built-in filtering)
// ---------------------------------------------------------------------------

/// `GET /agents` must NOT return built-in system agents.
#[tokio::test]
async fn test_list_agents_excludes_builtin_by_default() {
    let (app, storage, _manager, _tmp) = build_app().await;

    insert_builtin_agent(&storage, "agentd-system").await;
    let user = insert_user_agent(&storage, "my-user-agent").await;

    let response =
        app.oneshot(Request::builder().uri("/agents").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let items = json["items"].as_array().expect("expected items array");

    assert!(
        items.iter().all(|a| a["built_in"] != true),
        "built-in agents must be excluded from GET /agents by default"
    );
    assert!(
        items.iter().any(|a| a["id"] == user.id.to_string()),
        "user agent should appear in GET /agents"
    );
}

/// `GET /agents?include_builtin=true` returns both user and built-in agents.
#[tokio::test]
async fn test_list_agents_include_builtin_param() {
    let (app, storage, _manager, _tmp) = build_app().await;

    insert_builtin_agent(&storage, "agentd-system").await;
    insert_user_agent(&storage, "my-user-agent").await;

    let response = app
        .oneshot(
            Request::builder().uri("/agents?include_builtin=true").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let items = json["items"].as_array().expect("expected items array");
    assert_eq!(items.len(), 2, "should return both user and built-in agents");

    let has_builtin = items.iter().any(|a| a["built_in"] == true);
    let has_user = items.iter().any(|a| a["built_in"] != true);
    assert!(has_builtin, "should include built-in agent");
    assert!(has_user, "should include user agent");
}

// ---------------------------------------------------------------------------
// Tests: DELETE /agents/{id}
// ---------------------------------------------------------------------------

/// `DELETE /agents/{id}` must be blocked for built-in system agents.
#[tokio::test]
async fn test_delete_builtin_agent_is_rejected() {
    let (app, storage, _manager, _tmp) = build_app().await;
    let builtin = insert_builtin_agent(&storage, "agentd-system").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{}", builtin.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::FORBIDDEN
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "expected 400/403/422 when deleting a built-in agent, got {}",
        response.status()
    );
}

/// `DELETE /agents/{id}` still works for user agents (regression guard).
#[tokio::test]
async fn test_delete_user_agent_is_allowed() {
    let (app, storage, _manager, _tmp) = build_app().await;
    let user = insert_user_agent(&storage, "deletable-agent").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{}", user.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "deleting a user agent should succeed");
}

// ---------------------------------------------------------------------------
// Tests: POST /agents (built_in ignored)
// ---------------------------------------------------------------------------

/// `POST /agents` with `built_in: true` in the body must be ignored.
///
/// `built_in` is not part of `CreateAgentRequest`; any JSON value for that
/// key is silently discarded by serde's `deny_unknown_fields`-free defaults.
/// The created agent must have `built_in = false`.
#[tokio::test]
async fn test_create_agent_ignores_builtin_field() {
    let (app, _storage, _manager, _tmp) = build_app().await;

    let body = serde_json::json!({
        "name": "should-be-user-agent",
        "working_dir": "/tmp",
        // Attempt to set built_in via the API — must be ignored
        "built_in": true
    })
    .to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agents")
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    assert_eq!(
        json["built_in"],
        serde_json::Value::Bool(false),
        "built_in must default to false regardless of request body"
    );
}

// ---------------------------------------------------------------------------
// Tests: bootstrap_system_agents()
// ---------------------------------------------------------------------------

/// `bootstrap_system_agents()` creates the system agent with `built_in = true`.
#[tokio::test]
async fn test_bootstrap_creates_system_agent() {
    let (_app, storage, manager, _tmp) = build_app().await;

    manager.bootstrap_system_agents().await.unwrap();

    let system_agents = storage.list_system_agents().await.unwrap();
    assert_eq!(system_agents.len(), 1, "exactly one system agent should exist");

    let agent = &system_agents[0];
    assert_eq!(agent.name, SYSTEM_AGENT_NAME);
    assert!(agent.built_in, "system agent must have built_in = true");
}

/// `bootstrap_system_agents()` is idempotent — calling it twice does not
/// create a second system agent.
#[tokio::test]
async fn test_bootstrap_is_idempotent() {
    let (_app, storage, manager, _tmp) = build_app().await;

    manager.bootstrap_system_agents().await.unwrap();
    manager.bootstrap_system_agents().await.unwrap();

    let system_agents = storage.list_system_agents().await.unwrap();
    assert_eq!(system_agents.len(), 1, "bootstrap must not create duplicate system agents");
}

/// `AgentResponse` for a built-in agent includes `built_in = true`.
#[tokio::test]
async fn test_agent_response_includes_builtin_field() {
    let (app, storage, _manager, _tmp) = build_app().await;
    let builtin = insert_builtin_agent(&storage, "agentd-system").await;

    // GET /agents/{id} should expose the built_in field.
    let response = app
        .oneshot(
            Request::builder().uri(format!("/agents/{}", builtin.id)).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(
        json["built_in"],
        serde_json::Value::Bool(true),
        "GET /agents/{{id}} must return built_in = true for system agents"
    );
}
