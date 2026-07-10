//! Integration tests for `PATCH /agents/{id}` (general agent config update).
//!
//! Covers merge-patch semantics, the env redaction round-trip rules, the
//! `requires_restart` computation, and the built-in agent guard.
//!
//! # Design
//!
//! HTTP tests drive the full Axum router via `tower::ServiceExt::oneshot` with
//! no real TCP connection. A `NullBackend` replaces tmux so that
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
    types::{Agent, AgentConfig, AgentStatus},
    websocket::ConnectionRegistry,
};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;
use wrap::{
    backend::{SessionConfig, SessionExitInfo, SessionHealth},
    types::BackendType,
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

async fn build_app() -> (axum::Router, Arc<AgentStorage>, TempDir) {
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

    let state =
        ApiState { manager, registry, scheduler, communicate, backend_type: BackendType::Tmux };

    (create_router(state), storage, temp_dir)
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Insert an agent with the given status and env directly into storage.
async fn insert_agent(
    storage: &AgentStorage,
    name: &str,
    status: AgentStatus,
    env: HashMap<String, String>,
    built_in: bool,
) -> Agent {
    let mut config: AgentConfig =
        serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
    config.env = env;
    let mut agent = Agent::new(name.to_string(), config);
    agent.status = status;
    agent.built_in = built_in;
    storage.add(&agent).await.unwrap();
    agent
}

async fn patch_agent(
    app: axum::Router,
    id: Uuid,
    body: serde_json::Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("PATCH")
            .uri(format!("/agents/{id}"))
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_patch_nonexistent_agent_returns_404() {
    let (app, _storage, _tmp) = build_app().await;

    let response = patch_agent(app, Uuid::new_v4(), serde_json::json!({})).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_patch_builtin_agent_returns_403() {
    let (app, storage, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "sys", AgentStatus::Running, HashMap::new(), true).await;

    let response = patch_agent(app, agent.id, serde_json::json!({ "model": "opus" })).await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// Merge-patch semantics
// ---------------------------------------------------------------------------

/// An empty body is a valid no-op: nothing changes and no restart is needed.
#[tokio::test]
async fn test_patch_empty_body_is_noop() {
    let (app, storage, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "noop", AgentStatus::Running, HashMap::new(), false).await;

    let response = patch_agent(app, agent.id, serde_json::json!({})).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["requires_restart"], false);
    assert_eq!(json["restarted"], false);
    assert_eq!(json["name"], "noop");
}

/// Changing a launch-affecting field (model) on a running agent without
/// `restart: true` persists the config and flags `requires_restart`.
#[tokio::test]
async fn test_patch_model_on_running_agent_requires_restart() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "modeled", AgentStatus::Running, HashMap::new(), false).await;

    let response = patch_agent(app, agent.id, serde_json::json!({ "model": "opus" })).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["requires_restart"], true);
    assert_eq!(json["restarted"], false);
    assert_eq!(json["config"]["model"], "opus");

    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.model.as_deref(), Some("opus"));
}

/// Tool policy applies live — no restart required.
#[tokio::test]
async fn test_patch_tool_policy_does_not_require_restart() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "policied", AgentStatus::Running, HashMap::new(), false).await;

    let response = patch_agent(
        app,
        agent.id,
        serde_json::json!({ "tool_policy": { "mode": "require_approval" } }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["requires_restart"], false);
    assert_eq!(json["config"]["tool_policy"]["mode"], "require_approval");
}

/// Launch-affecting changes on a non-running agent need no restart flag —
/// they naturally apply the next time the agent starts.
#[tokio::test]
async fn test_patch_model_on_stopped_agent_no_restart_needed() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "stopped", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(app, agent.id, serde_json::json!({ "model": "haiku" })).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["requires_restart"], false);
}

/// With `restart: true` the agent is relaunched and the response says so.
#[tokio::test]
async fn test_patch_with_restart_relaunches_agent() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "relaunch", AgentStatus::Running, HashMap::new(), false).await;

    let response =
        patch_agent(app, agent.id, serde_json::json!({ "model": "opus", "restart": true })).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["restarted"], true);
    assert_eq!(json["requires_restart"], false);

    // The relaunch persisted the new config (delivered to the adapter via the
    // AAP initialize message) and regenerated the adapter launch command. Under
    // AAP the model is not a launch flag — it travels in `initialize` — so we
    // assert on the stored config and that the adapter is what gets launched.
    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.model.as_deref(), Some("opus"));
    let launch = stored.launch_command.unwrap_or_default();
    assert!(
        launch.contains("agentd-adapter-claude"),
        "launch command should invoke the adapter: {launch}"
    );
}

// ---------------------------------------------------------------------------
// Env redaction round-trip
// ---------------------------------------------------------------------------

/// `env` is a full replacement, except the `"***"` sentinel keeps the stored
/// value: a client can echo a redacted config back while adding and removing
/// keys, without ever learning the secrets.
#[tokio::test]
async fn test_patch_env_sentinel_round_trip() {
    let (app, storage, _tmp) = build_app().await;
    let mut env = HashMap::new();
    env.insert("A".to_string(), "secret-a".to_string());
    env.insert("B".to_string(), "secret-b".to_string());
    let agent = insert_agent(&storage, "envy", AgentStatus::Stopped, env, false).await;

    // Keep A (redacted sentinel), drop B (absent), add C (new value).
    let response =
        patch_agent(app, agent.id, serde_json::json!({ "env": { "A": "***", "C": "new-value" } }))
            .await;

    assert_eq!(response.status(), StatusCode::OK);
    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.env.get("A").map(String::as_str), Some("secret-a"));
    assert_eq!(stored.config.env.get("C").map(String::as_str), Some("new-value"));
    assert!(!stored.config.env.contains_key("B"), "omitted key must be removed");

    // The response itself is still redacted.
    let json = body_json(response).await;
    assert_eq!(json["config"]["env"]["A"], "***");
    assert_eq!(json["config"]["env"]["C"], "***");
}

/// A brand-new env key whose value is the redaction sentinel has no stored
/// value to preserve — reject rather than storing a literal "***".
#[tokio::test]
async fn test_patch_new_env_key_with_sentinel_returns_400() {
    let (app, storage, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "envy2", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(app, agent.id, serde_json::json!({ "env": { "NEW": "***" } })).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_patch_nonexistent_working_dir_returns_400() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "dirless", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(
        app,
        agent.id,
        serde_json::json!({ "working_dir": "/definitely/not/a/real/dir" }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_patch_empty_name_returns_400() {
    let (app, storage, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "named", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(app, agent.id, serde_json::json!({ "name": "   " })).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Setting a non-empty inline system prompt clears any file-based prompt,
/// and an empty string clears the inline prompt.
#[tokio::test]
async fn test_patch_system_prompt_mutual_exclusion_and_clearing() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "prompted", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(
        app.clone(),
        agent.id,
        serde_json::json!({ "system_prompt": "You are a test agent." }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.system_prompt.as_deref(), Some("You are a test agent."));
    assert_eq!(stored.config.system_prompt_file, None);

    let response = patch_agent(app, agent.id, serde_json::json!({ "system_prompt": "" })).await;
    assert_eq!(response.status(), StatusCode::OK);
    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.system_prompt, None, "empty string must clear the prompt");
}

// ---------------------------------------------------------------------------
// MCP server config (mcp_servers)
// ---------------------------------------------------------------------------

/// PATCH with mcp_servers persists the map and reports requires_restart on a
/// running agent.
#[tokio::test]
async fn test_patch_mcp_servers_persists_and_requires_restart() {
    let (app, storage, _tmp) = build_app().await;
    let agent = insert_agent(&storage, "mcp-me", AgentStatus::Running, HashMap::new(), false).await;

    let response = patch_agent(
        app,
        agent.id,
        serde_json::json!({
            "mcp_servers": {
                "agentd": { "command": "agent", "args": ["mcp"] }
            }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["requires_restart"], true, "mcp_servers is launch-affecting");

    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    let servers = stored.config.mcp_servers.expect("mcp_servers persisted");
    assert_eq!(servers["agentd"].command, "agent");
    assert_eq!(servers["agentd"].args, vec!["mcp"]);
}

/// An empty mcp_servers map clears the config.
#[tokio::test]
async fn test_patch_mcp_servers_empty_map_clears() {
    let (app, storage, _tmp) = build_app().await;
    let mut config: AgentConfig =
        serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
    config.mcp_servers = Some(
        serde_json::from_value(serde_json::json!({ "agentd": { "command": "agent" } })).unwrap(),
    );
    let mut agent = Agent::new("mcp-clear".to_string(), config);
    agent.status = AgentStatus::Stopped;
    storage.add(&agent).await.unwrap();

    let response = patch_agent(app, agent.id, serde_json::json!({ "mcp_servers": {} })).await;
    assert_eq!(response.status(), StatusCode::OK);

    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    assert_eq!(stored.config.mcp_servers, None, "empty map must clear mcp_servers");
}

/// MCP server env values are redacted in responses and round-trip via "***".
#[tokio::test]
async fn test_mcp_servers_env_redaction_round_trip() {
    let (app, storage, _tmp) = build_app().await;
    let mut config: AgentConfig =
        serde_json::from_value(serde_json::json!({ "working_dir": "/tmp" })).unwrap();
    config.mcp_servers = Some(
        serde_json::from_value(serde_json::json!({
            "agentd": { "command": "agent", "env": { "SECRET_TOKEN": "hunter2" } }
        }))
        .unwrap(),
    );
    let mut agent = Agent::new("mcp-secret".to_string(), config);
    agent.status = AgentStatus::Stopped;
    storage.add(&agent).await.unwrap();

    // GET must redact the env value.
    let response = app
        .clone()
        .oneshot(
            Request::builder().uri(format!("/agents/{}", agent.id)).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(
        json["config"]["mcp_servers"]["agentd"]["env"]["SECRET_TOKEN"], "***",
        "MCP env values must be redacted in responses"
    );

    // PATCH sending the sentinel back keeps the stored secret.
    let response = patch_agent(
        app,
        agent.id,
        serde_json::json!({
            "mcp_servers": {
                "agentd": { "command": "agent", "env": { "SECRET_TOKEN": "***" } }
            }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let stored = storage.get(&agent.id).await.unwrap().unwrap();
    let servers = stored.config.mcp_servers.unwrap();
    assert_eq!(
        servers["agentd"].env["SECRET_TOKEN"], "hunter2",
        "redaction sentinel must keep the stored value"
    );
}

/// The sentinel for a key with no stored value is rejected.
#[tokio::test]
async fn test_mcp_servers_redaction_sentinel_without_stored_value_rejected() {
    let (app, storage, _tmp) = build_app().await;
    let agent =
        insert_agent(&storage, "mcp-bad", AgentStatus::Stopped, HashMap::new(), false).await;

    let response = patch_agent(
        app,
        agent.id,
        serde_json::json!({
            "mcp_servers": {
                "agentd": { "command": "agent", "env": { "NEW_KEY": "***" } }
            }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// POST /agents with mcp_servers launches the AAP adapter (not `claude`
/// directly) and persists the servers for delivery via the AAP `initialize`
/// message. Under AAP the orchestrator no longer bakes `--mcp-config` into the
/// launch command or writes a per-agent MCP file — the adapter does that.
#[tokio::test]
async fn test_create_agent_with_mcp_servers_launches_adapter() {
    let (app, _storage, _tmp) = build_app().await;

    let body = serde_json::json!({
        "name": "mcp-agent",
        "working_dir": "/tmp",
        "mcp_servers": {
            "agentd": { "command": "agent", "args": ["mcp"] }
        }
    })
    .to_string();

    let response = app
        .clone()
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
    let id = json["id"].as_str().unwrap().to_string();

    // The launch command invokes the adapter with the AAP transport env, and
    // carries no Claude MCP flags.
    let launch_command = json["launch_command"].as_str().unwrap();
    assert!(launch_command.contains("agentd-adapter-claude"), "launch: {launch_command}");
    assert!(launch_command.contains("AGENTD_AAP_TRANSPORT"), "launch: {launch_command}");
    assert!(!launch_command.contains("--mcp-config"), "launch: {launch_command}");

    // The MCP servers are persisted (they travel in the AAP initialize message).
    assert_eq!(json["config"]["mcp_servers"]["agentd"]["command"], "agent");
    assert_eq!(json["config"]["agent_type"], "claude");

    // DELETE succeeds.
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// An empty command in an mcp_servers entry is rejected at create time.
#[tokio::test]
async fn test_create_agent_with_empty_mcp_command_rejected() {
    let (app, _storage, _tmp) = build_app().await;

    let body = serde_json::json!({
        "name": "mcp-bad-create",
        "working_dir": "/tmp",
        "mcp_servers": { "agentd": { "command": "  " } }
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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
