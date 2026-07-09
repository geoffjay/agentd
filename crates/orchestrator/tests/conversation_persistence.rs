//! End-to-end integration tests for the conversation persistence pipeline.
//!
//! Tests verify that conversation events flow correctly through the storage
//! layer and REST API, with correct ordering, filtering, pagination, and
//! deletion behaviour.
//!
//! # Coverage
//!
//! - Storage layer: cursor-based filtering (`since`/`until`), session
//!   filtering, ordering guarantees, cross-agent isolation.
//! - REST API: `GET /agents/{id}/conversation` with all query parameters,
//!   `GET /agents/{id}/conversation/summary`, single-event retrieval,
//!   `DELETE /agents/{id}/conversation`, and 404 error paths.
//!
//! # Design
//!
//! HTTP tests drive the full Axum router via `tower::ServiceExt::oneshot` with
//! no real TCP connection.  A `NullBackend` replaces tmux/Docker so that
//! `AgentManager` can be constructed without external dependencies.  All
//! databases are in a temp file (migrated automatically); no file descriptors
//! leak between tests because each test owns its own `TempDir`.

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use communicate::client::CommunicateClient;
use orchestrator::{
    api::{create_router, ApiState},
    manager::AgentManager,
    scheduler::{storage::SchedulerStorage, Scheduler},
    storage::AgentStorage,
    types::{
        Agent, AgentConfig, ConversationEvent, ConversationEventType, ConversationQuery, ToolPolicy,
    },
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

/// Build the full orchestrator API router backed by a temp-file database.
///
/// Returns the router, direct access to `AgentStorage`, and the `TempDir`
/// that **must** be kept alive for the duration of the test.
async fn build_app() -> (axum::Router, Arc<AgentStorage>, TempDir) {
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

    let state =
        ApiState { manager, registry, scheduler, communicate, backend_type: BackendType::Tmux };

    let router = create_router(state);
    (router, storage, temp_dir)
}

/// Create and persist a minimal agent, returning its UUID.
async fn create_agent(storage: &AgentStorage) -> Uuid {
    use std::collections::HashMap;
    let agent = Agent::new(
        format!("test-agent-{}", Uuid::new_v4()),
        AgentConfig {
            working_dir: "/tmp".to_string(),
            user: None,
            shell: "bash".to_string(),
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
            mcp_servers: None,
            agent_type: "claude".to_string(),
        },
    );
    storage.add(&agent).await.unwrap();
    agent.id
}

/// Convenience: insert one event of the given type for `agent_id` in session 0.
async fn insert_event(
    storage: &AgentStorage,
    agent_id: Uuid,
    event_type: ConversationEventType,
) -> ConversationEvent {
    let event = ConversationEvent::new(agent_id, event_type, 0, Some("hello".to_string()), None);
    storage.insert_conversation_event(&event).await.unwrap();
    event
}

/// Decode a response body as `serde_json::Value`.
async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// Storage layer: cursor filtering (since / until)
// ---------------------------------------------------------------------------

/// Verify that `ConversationQuery::since` excludes events older than the
/// threshold and includes events at or after it.
#[tokio::test]
async fn test_storage_since_filter() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    // Two events in the past, one "now".
    let past = Utc::now() - Duration::hours(2);
    let mut old_event = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("old".to_string()),
        None,
    );
    old_event.created_at = past;
    storage.insert_conversation_event(&old_event).await.unwrap();

    let mut older_event = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("older".to_string()),
        None,
    );
    older_event.created_at = past - Duration::hours(1);
    storage.insert_conversation_event(&older_event).await.unwrap();

    let recent = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("recent".to_string()),
        None,
    );
    storage.insert_conversation_event(&recent).await.unwrap();

    // Only events created at or after 90 minutes ago should be returned.
    let cutoff = Utc::now() - Duration::minutes(90);
    let opts = ConversationQuery { since: Some(cutoff), ..Default::default() };
    let events = storage.list_conversation_events(agent_id, &opts).await.unwrap();

    assert_eq!(events.len(), 1, "only the recent event should pass the since filter");
    assert_eq!(events[0].content.as_deref(), Some("recent"));
}

/// Verify that `ConversationQuery::until` excludes events at or after the
/// threshold.
#[tokio::test]
async fn test_storage_until_filter() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    let cutoff = Utc::now() - Duration::hours(1);

    // Event before the cutoff.
    let mut before = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("before".to_string()),
        None,
    );
    before.created_at = cutoff - Duration::minutes(30);
    storage.insert_conversation_event(&before).await.unwrap();

    // Event after the cutoff — must be excluded.
    let recent = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("after".to_string()),
        None,
    );
    storage.insert_conversation_event(&recent).await.unwrap();

    let opts = ConversationQuery { until: Some(cutoff), ..Default::default() };
    let events = storage.list_conversation_events(agent_id, &opts).await.unwrap();

    assert_eq!(events.len(), 1, "only the event before the cutoff should be returned");
    assert_eq!(events[0].content.as_deref(), Some("before"));
}

/// `since` and `until` can be combined to form a half-open time window.
#[tokio::test]
async fn test_storage_since_until_window() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    let now = Utc::now();

    // t-3h: too old
    let mut t1 = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("t1".to_string()),
        None,
    );
    t1.created_at = now - Duration::hours(3);
    storage.insert_conversation_event(&t1).await.unwrap();

    // t-2h: inside window
    let mut t2 = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("t2".to_string()),
        None,
    );
    t2.created_at = now - Duration::hours(2);
    storage.insert_conversation_event(&t2).await.unwrap();

    // t-1h: inside window
    let mut t3 = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("t3".to_string()),
        None,
    );
    t3.created_at = now - Duration::hours(1);
    storage.insert_conversation_event(&t3).await.unwrap();

    // Now: too new (until is exclusive)
    let t4 = ConversationEvent::new(
        agent_id,
        ConversationEventType::Output,
        0,
        Some("t4".to_string()),
        None,
    );
    storage.insert_conversation_event(&t4).await.unwrap();

    let opts = ConversationQuery {
        since: Some(now - Duration::hours(2) - Duration::seconds(1)),
        until: Some(now - Duration::seconds(1)),
        ..Default::default()
    };
    let events = storage.list_conversation_events(agent_id, &opts).await.unwrap();

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].content.as_deref(), Some("t2"));
    assert_eq!(events[1].content.as_deref(), Some("t3"));
}

/// Events are returned in ascending `created_at` order regardless of insertion
/// order.
#[tokio::test]
async fn test_storage_ordering_is_asc() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    let base = Utc::now() - Duration::hours(5);
    for i in [4i64, 1, 3, 0, 2] {
        let mut ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            i,
            Some(format!("event-{i}")),
            None,
        );
        ev.created_at = base + Duration::hours(i);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    let events =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();

    assert_eq!(events.len(), 5);
    // session_number encodes the insertion timestamp offset; verify strict ASC
    for window in events.windows(2) {
        assert!(
            window[0].created_at <= window[1].created_at,
            "events must be ordered by created_at ASC"
        );
    }
}

/// Session filtering: only events with a matching `session_number` are returned
/// when the caller supplies the `session` query param (tested at storage level
/// via direct field comparison).
#[tokio::test]
async fn test_storage_session_isolation() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    for session in [0i64, 0, 1, 1, 2] {
        let event =
            ConversationEvent::new(agent_id, ConversationEventType::Output, session, None, None);
        storage.insert_conversation_event(&event).await.unwrap();
    }

    let all =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();
    assert_eq!(all.len(), 5);

    // Filter in application layer as the API does.
    let session_1: Vec<_> = all.iter().filter(|e| e.session_number == 1).collect();
    assert_eq!(session_1.len(), 2);

    let session_2: Vec<_> = all.iter().filter(|e| e.session_number == 2).collect();
    assert_eq!(session_2.len(), 1);
}

/// Deleting one agent's events must not affect another agent's events.
#[tokio::test]
async fn test_storage_delete_cross_agent_isolation() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();

    for _ in 0..3 {
        insert_event(&storage, agent_a, ConversationEventType::Output).await;
    }
    for _ in 0..2 {
        insert_event(&storage, agent_b, ConversationEventType::Output).await;
    }

    let deleted = storage.delete_conversation_events_for_agent(agent_a).await.unwrap();
    assert_eq!(deleted, 3);
    assert_eq!(storage.count_conversation_events(agent_a).await.unwrap(), 0);
    assert_eq!(storage.count_conversation_events(agent_b).await.unwrap(), 2);
}

/// All eight `ConversationEventType` variants round-trip through storage with
/// the correct discriminator string.
#[tokio::test]
async fn test_storage_all_event_types_roundtrip() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    let types = [
        ConversationEventType::Output,
        ConversationEventType::ToolUse,
        ConversationEventType::Thinking,
        ConversationEventType::Result,
        ConversationEventType::PromptSent,
        ConversationEventType::ActivityChanged,
        ConversationEventType::UsageUpdate,
        ConversationEventType::ContextCleared,
    ];

    for et in types.iter().cloned() {
        insert_event(&storage, agent_id, et).await;
    }

    let events =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();

    assert_eq!(events.len(), types.len());
    // The API serialises event_type as "agent:<variant>"; verify the storage
    // enum variant round-trips correctly.
    for (ev, expected) in events.iter().zip(types.iter()) {
        assert_eq!(ev.event_type, *expected);
    }
}

/// Metadata stored as a JSON value is retrieved unchanged.
#[tokio::test]
async fn test_storage_metadata_roundtrip() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    let meta = serde_json::json!({
        "tool_name": "Bash",
        "tool_id": "abc123",
        "tool_input": {"command": "ls -la"},
        "summary": "list files"
    });
    let event = ConversationEvent::new(
        agent_id,
        ConversationEventType::ToolUse,
        0,
        None,
        Some(meta.clone()),
    );
    storage.insert_conversation_event(&event).await.unwrap();

    let events =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].metadata, Some(meta));
}

// ---------------------------------------------------------------------------
// REST API: GET /agents/{id}/conversation
// ---------------------------------------------------------------------------

/// `GET /agents/{id}/conversation` returns all events for the agent.
#[tokio::test]
async fn test_api_get_conversation_returns_events() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    for _ in 0..3 {
        insert_event(&storage, agent_id, ConversationEventType::Output).await;
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
}

/// `GET /agents/{id}/conversation` with `?limit=2` returns only 2 events and
/// sets `has_more = true` when more exist.
#[tokio::test]
async fn test_api_get_conversation_limit() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    for _ in 0..5 {
        insert_event(&storage, agent_id, ConversationEventType::Output).await;
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation?limit=2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["events"].as_array().unwrap().len(), 2);
    assert_eq!(json["has_more"], true, "has_more must be true when limit truncates results");
}

/// `GET /agents/{id}/conversation?event_type=output` returns only output events.
#[tokio::test]
async fn test_api_get_conversation_event_type_filter() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    insert_event(&storage, agent_id, ConversationEventType::Output).await;
    insert_event(&storage, agent_id, ConversationEventType::ToolUse).await;
    insert_event(&storage, agent_id, ConversationEventType::Thinking).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation?event_type=output"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    // The REST API serialises `event_type` as `"type"` to match the WebSocket
    // stream event format (see ConversationEventResponse's serde rename).
    assert_eq!(events[0]["type"], "agent:output");
}

/// `GET /agents/{id}/conversation?session=1` returns only events from session 1.
#[tokio::test]
async fn test_api_get_conversation_session_filter() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    // Session 0: 2 events
    for _ in 0..2 {
        let ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            0,
            Some("s0".to_string()),
            None,
        );
        storage.insert_conversation_event(&ev).await.unwrap();
    }
    // Session 1: 3 events
    for _ in 0..3 {
        let ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            1,
            Some("s1".to_string()),
            None,
        );
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation?session=1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 3, "should return only session-1 events");
}

/// `GET /agents/{id}/conversation?after=<ts>` returns only events after the
/// timestamp.
#[tokio::test]
async fn test_api_get_conversation_after_cursor() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let base = Utc::now() - Duration::hours(3);

    // 3 old events
    for i in 0..3 {
        let mut ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            0,
            Some(format!("old-{i}")),
            None,
        );
        ev.created_at = base + Duration::hours(i);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    // 2 recent events
    for i in 0..2 {
        let mut ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            0,
            Some(format!("new-{i}")),
            None,
        );
        ev.created_at = Utc::now() + Duration::seconds(i);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    let cutoff = (base + Duration::hours(3)).to_rfc3339();
    let uri = format!("/agents/{agent_id}/conversation?after={}", urlencoding::encode(&cutoff));

    let response =
        app.oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 2, "only events after the cursor should be returned");
}

/// `GET /agents/{id}/conversation?before=<ts>` returns only events before the
/// timestamp.
#[tokio::test]
async fn test_api_get_conversation_before_cursor() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let base = Utc::now() - Duration::hours(2);

    for i in 0..4 {
        let mut ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            0,
            Some(format!("event-{i}")),
            None,
        );
        ev.created_at = base + Duration::minutes(i * 15);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    // Set cutoff after the first 2 events.
    let cutoff = (base + Duration::minutes(30)).to_rfc3339();
    let uri = format!("/agents/{agent_id}/conversation?before={}", urlencoding::encode(&cutoff));

    let response =
        app.oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 2, "only events before the cursor should be returned");
}

/// `GET /agents/{nonexistent}/conversation` returns 404.
#[tokio::test]
async fn test_api_get_conversation_unknown_agent_404() {
    let (app, _storage, _tmp) = build_app().await;
    let unknown_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{unknown_id}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// `GET /agents/{id}/conversation` for an agent with no events returns an empty
/// list with `total = 0` and `has_more = false`.
#[tokio::test]
async fn test_api_get_conversation_empty() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert!(json["events"].as_array().unwrap().is_empty());
    assert_eq!(json["total"], 0);
    assert_eq!(json["has_more"], false);
}

// ---------------------------------------------------------------------------
// REST API: GET /agents/{id}/conversation/summary
// ---------------------------------------------------------------------------

/// Summary returns correct totals, per-type counts, and session count.
#[tokio::test]
async fn test_api_get_conversation_summary() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    // 3 Output in session 0, 2 ToolUse in session 1.
    for _ in 0..3 {
        let ev = ConversationEvent::new(agent_id, ConversationEventType::Output, 0, None, None);
        storage.insert_conversation_event(&ev).await.unwrap();
    }
    for _ in 0..2 {
        let ev = ConversationEvent::new(agent_id, ConversationEventType::ToolUse, 1, None, None);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation/summary"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total_events"], 5);
    assert_eq!(json["session_count"], 2);
    assert_eq!(json["event_counts"]["agent:output"], 3);
    assert_eq!(json["event_counts"]["agent:tool_use"], 2);
}

/// Summary for an unknown agent returns 404.
#[tokio::test]
async fn test_api_get_conversation_summary_unknown_agent_404() {
    let (app, _storage, _tmp) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{}/conversation/summary", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// REST API: GET /agents/{id}/conversation/{event_id}
// ---------------------------------------------------------------------------

/// Single-event endpoint returns the correct event.
#[tokio::test]
async fn test_api_get_single_event() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let event = insert_event(&storage, agent_id, ConversationEventType::ToolUse).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation/{}", event.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["id"], event.id.to_string());
    // The REST API serialises `event_type` as `"type"` to match the WebSocket
    // stream event format (see ConversationEventResponse's serde rename).
    assert_eq!(json["type"], "agent:tool_use");
}

/// Fetching an event from a different agent returns 404.
#[tokio::test]
async fn test_api_get_single_event_wrong_agent_404() {
    let (app, storage, _tmp) = build_app().await;
    let agent_a = create_agent(&storage).await;
    let agent_b = create_agent(&storage).await;

    let event = insert_event(&storage, agent_a, ConversationEventType::Output).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_b}/conversation/{}", event.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Fetching a non-existent event ID returns 404.
#[tokio::test]
async fn test_api_get_single_event_unknown_event_id_404() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/agents/{agent_id}/conversation/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// REST API: DELETE /agents/{id}/conversation
// ---------------------------------------------------------------------------

/// `DELETE /agents/{id}/conversation` removes all events and returns 204.
#[tokio::test]
async fn test_api_delete_conversation_clears_history() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    for _ in 0..4 {
        insert_event(&storage, agent_id, ConversationEventType::Output).await;
    }

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{agent_id}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(storage.count_conversation_events(agent_id).await.unwrap(), 0);
}

/// `DELETE /agents/{id}/conversation` on an unknown agent returns 404.
#[tokio::test]
async fn test_api_delete_conversation_unknown_agent_404() {
    let (app, _storage, _tmp) = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{}/conversation", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// `DELETE /agents/{id}/conversation` does not remove events belonging to other
/// agents.
#[tokio::test]
async fn test_api_delete_conversation_cross_agent_isolation() {
    let (app, storage, _tmp) = build_app().await;
    let agent_a = create_agent(&storage).await;
    let agent_b = create_agent(&storage).await;

    for _ in 0..3 {
        insert_event(&storage, agent_a, ConversationEventType::Output).await;
    }
    for _ in 0..2 {
        insert_event(&storage, agent_b, ConversationEventType::Output).await;
    }

    // Delete only agent_a's history.
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{agent_a}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    assert_eq!(storage.count_conversation_events(agent_a).await.unwrap(), 0);
    assert_eq!(storage.count_conversation_events(agent_b).await.unwrap(), 2);
}

/// `DELETE /agents/{id}/conversation` is idempotent — deleting when no events
/// exist still returns 204.
#[tokio::test]
async fn test_api_delete_conversation_idempotent() {
    let (app, storage, _tmp) = build_app().await;
    let agent_id = create_agent(&storage).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/agents/{agent_id}/conversation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// Count accuracy
// ---------------------------------------------------------------------------

/// `count_conversation_events` matches the number of events returned by list.
#[tokio::test]
async fn test_count_matches_list_length() {
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    const N: usize = 7;
    for _ in 0..N {
        insert_event(&storage, agent_id, ConversationEventType::Output).await;
    }

    let count = storage.count_conversation_events(agent_id).await.unwrap();
    let list =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();

    assert_eq!(count as usize, list.len());
    assert_eq!(count as usize, N);
}
