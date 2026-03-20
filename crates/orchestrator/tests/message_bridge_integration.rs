//! Integration tests for the orchestrator message bridge.
//!
//! Tests the critical integration between the orchestrator and the communicate
//! service — specifically the message-to-prompt bridge that delivers room
//! messages to agents and posts agent responses back to rooms.
//!
//! # Infrastructure
//!
//! An in-process mock communicate service is built with `axum` (already a
//! dependency). It provides:
//!
//! - HTTP REST endpoints: `GET /rooms/{id}`, `POST /rooms/{id}/participants`,
//!   `POST /rooms/{id}/messages`
//! - WebSocket endpoint: `GET /ws` for the bridge to subscribe to room events
//!
//! Mock agent connections use `tokio::sync::mpsc` channels so tests can read
//! prompts that the bridge delivers to agents.

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use communicate::{
    client::CommunicateClient,
    types::{
        AddParticipantRequest, CreateMessageRequest, MessageResponse, MessageStatus,
        ParticipantKind, ParticipantResponse, RoomResponse, RoomType,
    },
};
use orchestrator::{
    message_bridge::MessageBridge,
    scheduler::events::{EventBus, SystemEvent},
    storage::AgentStorage,
    types::ResultInfo,
    websocket::{AgentConnection, ConnectionRegistry},
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex, Notify, RwLock};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Mock communicate server
// ---------------------------------------------------------------------------

/// Shared state for the mock communicate service.
#[derive(Clone)]
struct MockState {
    /// Rooms stored by ID.
    rooms: Arc<RwLock<HashMap<Uuid, RoomResponse>>>,
    /// Participant additions recorded by test assertions.
    add_participant_calls: Arc<Mutex<Vec<(Uuid, AddParticipantRequest)>>>,
    /// Messages posted to rooms recorded by test assertions.
    sent_messages: Arc<Mutex<Vec<(Uuid, CreateMessageRequest)>>>,
    /// Broadcast to push JSON text FROM the test TO the bridge WebSocket.
    bridge_push_tx: broadcast::Sender<String>,
    /// Messages received FROM the bridge WebSocket (subscribe commands, etc.).
    bridge_recv: Arc<Mutex<Vec<String>>>,
    /// Notified once when the bridge WebSocket connects.
    bridge_connected: Arc<Notify>,
}

impl MockState {
    fn new(bridge_push_tx: broadcast::Sender<String>) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            add_participant_calls: Arc::new(Mutex::new(Vec::new())),
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            bridge_push_tx,
            bridge_recv: Arc::new(Mutex::new(Vec::new())),
            bridge_connected: Arc::new(Notify::new()),
        }
    }
}

// --- HTTP route handlers ---

async fn get_room_handler(
    Path(room_id): Path<Uuid>,
    State(state): State<MockState>,
) -> impl IntoResponse {
    let rooms = state.rooms.read().await;
    match rooms.get(&room_id) {
        Some(room) => Json(room.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn add_participant_handler(
    Path(room_id): Path<Uuid>,
    State(state): State<MockState>,
    axum::extract::Json(req): axum::extract::Json<AddParticipantRequest>,
) -> impl IntoResponse {
    state.add_participant_calls.lock().await.push((room_id, req.clone()));
    let resp = ParticipantResponse {
        id: Uuid::new_v4(),
        room_id,
        identifier: req.identifier.clone(),
        kind: req.kind,
        display_name: req.display_name,
        role: req.role,
        joined_at: Utc::now(),
    };
    (StatusCode::CREATED, Json(resp)).into_response()
}

async fn send_message_handler(
    Path(room_id): Path<Uuid>,
    State(state): State<MockState>,
    axum::extract::Json(req): axum::extract::Json<CreateMessageRequest>,
) -> impl IntoResponse {
    state.sent_messages.lock().await.push((room_id, req.clone()));
    let resp = MessageResponse {
        id: Uuid::new_v4(),
        room_id,
        sender_id: req.sender_id,
        sender_name: req.sender_name,
        sender_kind: req.sender_kind,
        content: req.content,
        metadata: req.metadata,
        reply_to: req.reply_to,
        status: MessageStatus::Sent,
        created_at: Utc::now(),
    };
    (StatusCode::CREATED, Json(resp)).into_response()
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({"status": "ok", "service": "mock-communicate"}))
}

async fn mock_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<MockState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_mock_ws(socket, state))
}

async fn handle_mock_ws(mut socket: WebSocket, state: MockState) {
    state.bridge_connected.notify_one();
    let mut push_rx = state.bridge_push_tx.subscribe();

    loop {
        tokio::select! {
            // Messages FROM the bridge (subscribe commands, etc.)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        state.bridge_recv.lock().await.push(text.to_string());
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
            // Messages TO push to the bridge
            pushed = push_rx.recv() => {
                match pushed {
                    Ok(text) => {
                        if socket.send(WsMessage::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

// --- Server startup ---

/// Start the mock communicate server. Returns (`MockState`, push-sender, base URL).
async fn start_mock_server() -> (MockState, broadcast::Sender<String>, String) {
    let (bridge_push_tx, _) = broadcast::channel::<String>(64);
    let state = MockState::new(bridge_push_tx.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let app = Router::new()
        .route("/rooms/{id}", get(get_room_handler))
        .route("/rooms/{id}/participants", post(add_participant_handler))
        .route("/rooms/{id}/messages", post(send_message_handler))
        .route("/ws", get(mock_ws_handler))
        .route("/health", get(health_handler))
        .with_state(state.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (state, bridge_push_tx, base_url)
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Create a minimal `RoomResponse` for tests.
fn make_room(id: Uuid, name: &str, room_type: RoomType) -> RoomResponse {
    let now = Utc::now();
    RoomResponse {
        id,
        name: name.to_string(),
        topic: None,
        description: None,
        room_type,
        created_by: "test-creator".to_string(),
        created_at: now,
        updated_at: now,
    }
}

/// Build a `MessageResponse` representing a message posted to a room.
fn make_message(
    room_id: Uuid,
    sender_id: &str,
    sender_name: &str,
    sender_kind: ParticipantKind,
    content: &str,
) -> MessageResponse {
    MessageResponse {
        id: Uuid::new_v4(),
        room_id,
        sender_id: sender_id.to_string(),
        sender_name: sender_name.to_string(),
        sender_kind,
        content: content.to_string(),
        metadata: HashMap::new(),
        reply_to: None,
        status: MessageStatus::Sent,
        created_at: Utc::now(),
    }
}

/// Create an `AgentStorage` backed by a temp-file SQLite database.
///
/// The caller must hold `_tmp` alive for the duration of the test.
async fn create_agent_storage() -> (AgentStorage, TempDir) {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("test.db");
    let storage = AgentStorage::with_path(&db_path).await.unwrap();
    (storage, tmp)
}

/// Register a mock agent in `registry`.
///
/// Returns `(agent_id, mpsc_receiver)`. The receiver yields every prompt the
/// bridge delivers to the agent.
async fn register_mock_agent(
    registry: &ConnectionRegistry,
) -> (Uuid, mpsc::UnboundedReceiver<String>) {
    let agent_id = Uuid::new_v4();
    let (tx, rx) = mpsc::unbounded_channel::<String>();
    registry.register(agent_id, AgentConnection { tx }).await;
    (agent_id, rx)
}

/// Build a `MessageBridge` pointed at `base_url`.
///
/// Returns `(Arc<MessageBridge>, TempDir)`. The `TempDir` must be held for the
/// test lifetime to keep the SQLite file alive.
async fn create_bridge(
    registry: ConnectionRegistry,
    event_bus: Arc<EventBus>,
    base_url: &str,
) -> (Arc<MessageBridge>, TempDir) {
    let (storage, tmp) = create_agent_storage().await;
    let communicate = CommunicateClient::new_no_proxy(base_url);
    let bridge =
        Arc::new(MessageBridge::new(registry, communicate, Arc::new(storage), event_bus, base_url));
    (bridge, tmp)
}

/// Poll `check` every 50 ms until it returns `true` or `timeout_ms` elapses.
async fn wait_until<F, Fut>(mut check: F, timeout_ms: u64) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if check().await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Push a server message (room message event) over the mock WS to the bridge.
fn push_room_message(bridge_push_tx: &broadcast::Sender<String>, msg: &MessageResponse) {
    let payload = json!({
        "type": "message",
        "room_id": msg.room_id,
        "message": {
            "id": msg.id,
            "room_id": msg.room_id,
            "sender_id": msg.sender_id,
            "sender_name": msg.sender_name,
            "sender_kind": msg.sender_kind.to_string(),
            "content": msg.content,
            "metadata": msg.metadata,
            "reply_to": msg.reply_to,
            "status": "sent",
            "created_at": msg.created_at.to_rfc3339(),
        }
    });
    let _ = bridge_push_tx.send(payload.to_string());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Agent auto-join: when `AgentJoinedRoom` is published, the bridge fetches
/// the room from the communicate service and subscribes to it via WebSocket.
#[tokio::test]
async fn test_agent_auto_join_subscribes_to_room() {
    let (state, _push_tx, base_url) = start_mock_server().await;

    // Seed the room the agent will join.
    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "ops", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();
    let (agent_id, _rx) = register_mock_agent(&registry).await;
    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;

    // Allow bridge tasks to start.
    sleep(Duration::from_millis(100)).await;

    // Trigger the join.
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id, room_id });

    // Wait for the bridge to call add_participant (proves on_agent_joined_room ran).
    let participant_added = wait_until(
        || {
            let calls = state.add_participant_calls.clone();
            async move { !calls.lock().await.is_empty() }
        },
        3000,
    )
    .await;
    assert!(participant_added, "bridge should add itself as participant in the room");

    // Verify the bridge sent a subscribe message over WebSocket.
    let subscribe_received = wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move {
                recv.lock()
                    .await
                    .iter()
                    .any(|msg| msg.contains("subscribe") && msg.contains(&room_id.to_string()))
            }
        },
        3000,
    )
    .await;
    assert!(subscribe_received, "bridge should send a subscribe message for the room");
}

/// Message delivery: a message posted to a room is delivered as a prompt to the
/// agent via its WebSocket (mpsc channel in tests).
#[tokio::test]
async fn test_message_delivered_to_agent_via_ws() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "general", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();
    let (agent_id, mut agent_rx) = register_mock_agent(&registry).await;
    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;

    sleep(Duration::from_millis(100)).await;

    // Join the room so bridge subscribes.
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id, room_id });

    // Wait for subscription.
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { !recv.lock().await.is_empty() }
        },
        3000,
    )
    .await;

    // Push a room message through the mock WS.
    let msg = make_message(room_id, "human-1", "Alice", ParticipantKind::Human, "Hello agent!");
    push_room_message(&push_tx, &msg);

    // The bridge should deliver a prompt to the agent's mpsc channel.
    let received = wait_until(
        || {
            // Try to read from the channel without blocking.
            // We use a shared Arc to do the non-blocking try_recv inside an async fn.
            let _ = &agent_rx; // borrow check: agent_rx outlives this closure
            async { false } // placeholder — handled below with real try_recv
        },
        3000,
    )
    .await;

    // Directly poll the channel with a small sleep loop (try_recv is sync).
    let prompt = {
        let mut result = None;
        for _ in 0..60 {
            if let Ok(msg) = agent_rx.try_recv() {
                result = Some(msg);
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        result
    };

    let prompt = prompt.expect("bridge should have delivered a prompt to the agent");
    let parsed: serde_json::Value = serde_json::from_str(&prompt.trim()).unwrap();
    assert_eq!(parsed["type"], "user", "should be a user message");
    let content = parsed["message"]["content"].as_str().unwrap();
    assert!(content.contains("general"), "prompt should include room name");
    assert!(content.contains("Alice"), "prompt should include sender name");
    assert!(content.contains("Hello agent!"), "prompt should include message content");
    let _ = received;
}

/// Agent response: when the agent produces a result, the bridge posts the
/// response back to the communicate room.
#[tokio::test]
async fn test_agent_response_posted_to_room() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "responses", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();
    let (agent_id, mut agent_rx) = register_mock_agent(&registry).await;
    let (bridge, _tmp) = create_bridge(registry.clone(), event_bus.clone(), &base_url).await;
    bridge.clone().start().await;

    sleep(Duration::from_millis(100)).await;

    // Join room.
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id, room_id });
    wait_until(
        || {
            let calls = state.add_participant_calls.clone();
            async move { !calls.lock().await.is_empty() }
        },
        3000,
    )
    .await;

    // Wait for subscription.
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { !recv.lock().await.is_empty() }
        },
        3000,
    )
    .await;

    // Deliver a room message so the bridge records the active room.
    let msg = make_message(room_id, "human-1", "Alice", ParticipantKind::Human, "What is 2+2?");
    push_room_message(&push_tx, &msg);

    // Wait for agent to receive the prompt (agent is now Busy with active room set).
    for _ in 0..60 {
        if agent_rx.try_recv().is_ok() {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    // Simulate the agent producing a result — the registry must set Idle first
    // (this mirrors what the real WS handler does when it receives "result").
    // In tests we call notify_result which triggers the bridge callback.
    let result = ResultInfo {
        agent_id,
        is_error: false,
        usage: None,
        result_text: "The answer is 4.".to_string(),
    };
    registry.notify_result(result).await;

    // Wait for the bridge to POST the response to the room.
    let message_posted = wait_until(
        || {
            let msgs = state.sent_messages.clone();
            async move { !msgs.lock().await.is_empty() }
        },
        3000,
    )
    .await;
    assert!(message_posted, "bridge should post agent response to the communicate room");

    let messages = state.sent_messages.lock().await;
    assert!(!messages.is_empty());
    let (posted_room_id, posted_msg) = &messages[0];
    assert_eq!(*posted_room_id, room_id);
    assert!(
        posted_msg.content.contains("The answer is 4."),
        "posted content should include agent result: {}",
        posted_msg.content,
    );
    // Echo-prevention metadata must be set.
    assert_eq!(
        posted_msg.metadata.get("source").map(|s| s.as_str()),
        Some("agent_response"),
        "response message must have source=agent_response metadata",
    );
}

/// Echo prevention: a message with `source=agent_response` metadata must NOT
/// be re-delivered to the agent that produced it.
#[tokio::test]
async fn test_echo_prevention_full_flow() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "echo-test", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();
    let (agent_id, mut agent_rx) = register_mock_agent(&registry).await;
    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;

    sleep(Duration::from_millis(100)).await;

    // Join the room.
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id, room_id });
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { !recv.lock().await.is_empty() }
        },
        3000,
    )
    .await;

    // Push a message that carries the echo-prevention metadata.
    let mut echo_msg =
        make_message(room_id, &agent_id.to_string(), "Bot", ParticipantKind::Agent, "I replied");
    echo_msg.metadata.insert("source".to_string(), "agent_response".to_string());

    let payload = json!({
        "type": "message",
        "room_id": room_id,
        "message": {
            "id": echo_msg.id,
            "room_id": echo_msg.room_id,
            "sender_id": echo_msg.sender_id,
            "sender_name": echo_msg.sender_name,
            "sender_kind": echo_msg.sender_kind.to_string(),
            "content": echo_msg.content,
            "metadata": echo_msg.metadata,
            "reply_to": null,
            "status": "sent",
            "created_at": echo_msg.created_at.to_rfc3339(),
        }
    });
    let _ = push_tx.send(payload.to_string());

    // Wait briefly — the bridge must NOT deliver anything to the agent.
    sleep(Duration::from_millis(300)).await;

    assert!(
        agent_rx.try_recv().is_err(),
        "echo-prevention: bridge must not deliver agent_response messages back to the agent"
    );
}

/// Direct room: in a Direct room only the non-sender participant receives the
/// message.  The sender is excluded even if it is an agent participant.
#[tokio::test]
async fn test_direct_room_only_delivers_to_non_sender() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "dm", RoomType::Direct));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();

    // Register two agents.
    let (agent_a, mut rx_a) = register_mock_agent(&registry).await;
    let (agent_b, mut rx_b) = register_mock_agent(&registry).await;

    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;
    sleep(Duration::from_millis(100)).await;

    // Both agents join the direct room.
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id: agent_a, room_id });
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id: agent_b, room_id });

    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { recv.lock().await.iter().filter(|m| m.contains("subscribe")).count() >= 1 }
        },
        3000,
    )
    .await;
    sleep(Duration::from_millis(150)).await;

    // Push a message sent by agent_a.
    let msg =
        make_message(room_id, &agent_a.to_string(), "Agent A", ParticipantKind::Agent, "Direct DM");
    push_room_message(&push_tx, &msg);

    sleep(Duration::from_millis(300)).await;

    // agent_a (sender) must NOT receive the message.
    assert!(rx_a.try_recv().is_err(), "sender agent_a must not receive its own direct message");

    // agent_b (recipient) MUST receive the message.
    let received_by_b = rx_b.try_recv().is_ok();
    assert!(received_by_b, "recipient agent_b must receive the direct message");
}

/// Group room: all agent participants (except the sender) receive the message.
#[tokio::test]
async fn test_group_room_delivers_to_all_agents() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "group-chat", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();

    let (agent_a, mut rx_a) = register_mock_agent(&registry).await;
    let (agent_b, mut rx_b) = register_mock_agent(&registry).await;
    let (agent_c, mut rx_c) = register_mock_agent(&registry).await;

    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;
    sleep(Duration::from_millis(100)).await;

    for &id in &[agent_a, agent_b, agent_c] {
        event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id: id, room_id });
    }

    // Wait for at least one subscribe message.
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { recv.lock().await.iter().any(|m| m.contains("subscribe")) }
        },
        3000,
    )
    .await;
    sleep(Duration::from_millis(200)).await;

    // A human sends a message to the group.
    let msg = make_message(room_id, "human-1", "Human", ParticipantKind::Human, "Hello group!");
    push_room_message(&push_tx, &msg);

    sleep(Duration::from_millis(400)).await;

    // All three agents must receive a prompt.
    let got_a = rx_a.try_recv().is_ok();
    let got_b = rx_b.try_recv().is_ok();
    let got_c = rx_c.try_recv().is_ok();

    assert!(got_a, "agent_a must receive the group message");
    assert!(got_b, "agent_b must receive the group message");
    assert!(got_c, "agent_c must receive the group message");
}

/// Multi-agent room: a message from agent A is delivered to agents B and C,
/// but not back to A.
#[tokio::test]
async fn test_multi_agent_room_agent_sender_excluded() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state.rooms.write().await.insert(room_id, make_room(room_id, "multi", RoomType::Group));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();

    let (agent_a, mut rx_a) = register_mock_agent(&registry).await;
    let (agent_b, mut rx_b) = register_mock_agent(&registry).await;
    let (agent_c, mut rx_c) = register_mock_agent(&registry).await;

    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;
    sleep(Duration::from_millis(100)).await;

    for &id in &[agent_a, agent_b, agent_c] {
        event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id: id, room_id });
    }
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { recv.lock().await.iter().any(|m| m.contains("subscribe")) }
        },
        3000,
    )
    .await;
    sleep(Duration::from_millis(200)).await;

    // Agent A sends a message.
    let msg = make_message(
        room_id,
        &agent_a.to_string(),
        "Agent A",
        ParticipantKind::Agent,
        "From A to all",
    );
    push_room_message(&push_tx, &msg);

    sleep(Duration::from_millis(400)).await;

    // Agent A must NOT receive its own message.
    assert!(rx_a.try_recv().is_err(), "agent_a (sender) must not receive its own message");

    // Agents B and C must receive the message.
    assert!(rx_b.try_recv().is_ok(), "agent_b must receive message from agent_a");
    assert!(rx_c.try_recv().is_ok(), "agent_c must receive message from agent_a");
}

/// Graceful degradation: the bridge starts successfully and does not panic when
/// the communicate service is unavailable.  `AgentJoinedRoom` events are handled
/// without crashing the orchestrator.
#[tokio::test]
async fn test_graceful_degradation_communicate_unavailable() {
    // Point bridge at a port nothing is listening on.
    let unreachable_url = "http://127.0.0.1:19999";

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();
    let (agent_id, _rx) = register_mock_agent(&registry).await;

    let (storage, _tmp) = create_agent_storage().await;
    let communicate = CommunicateClient::new_no_proxy(unreachable_url);
    let bridge = Arc::new(MessageBridge::new(
        registry,
        communicate,
        Arc::new(storage),
        event_bus.clone(),
        unreachable_url,
    ));
    // start() should not panic even with an unreachable communicate service.
    bridge.clone().start().await;

    sleep(Duration::from_millis(100)).await;

    // Publishing a join event on an unreachable communicate service must not panic.
    let room_id = Uuid::new_v4();
    event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id, room_id });

    // Give the bridge time to attempt (and fail) the HTTP call.
    sleep(Duration::from_millis(300)).await;

    // Test passes if we reach here without panicking.
}

/// Test helper: create rooms and seed them into a mock state for programmatic setup.
#[tokio::test]
async fn test_helper_creates_room_and_participants() {
    let (state, _push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    let room = make_room(room_id, "test-room", RoomType::Group);
    state.rooms.write().await.insert(room_id, room.clone());

    // Verify via HTTP that the mock serves the room correctly.
    let client = CommunicateClient::new_no_proxy(&base_url);
    let fetched = client.get_room(room_id).await.unwrap();
    assert!(fetched.is_some(), "mock server should return the seeded room");
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, room_id);
    assert_eq!(fetched.name, "test-room");
    assert_eq!(fetched.room_type, RoomType::Group);
}

/// Broadcast room: delivers to all agent participants (same semantics as Group).
#[tokio::test]
async fn test_broadcast_room_delivers_to_all_agents() {
    let (state, push_tx, base_url) = start_mock_server().await;

    let room_id = Uuid::new_v4();
    state
        .rooms
        .write()
        .await
        .insert(room_id, make_room(room_id, "broadcast-ch", RoomType::Broadcast));

    let event_bus = EventBus::shared(16);
    let registry = ConnectionRegistry::new();

    let (agent_a, mut rx_a) = register_mock_agent(&registry).await;
    let (agent_b, mut rx_b) = register_mock_agent(&registry).await;

    let (bridge, _tmp) = create_bridge(registry, event_bus.clone(), &base_url).await;
    bridge.clone().start().await;
    sleep(Duration::from_millis(100)).await;

    for &id in &[agent_a, agent_b] {
        event_bus.publish(SystemEvent::AgentJoinedRoom { agent_id: id, room_id });
    }
    wait_until(
        || {
            let recv = state.bridge_recv.clone();
            async move { recv.lock().await.iter().any(|m| m.contains("subscribe")) }
        },
        3000,
    )
    .await;
    sleep(Duration::from_millis(200)).await;

    let msg =
        make_message(room_id, "human-1", "Announcer", ParticipantKind::Human, "Broadcast message");
    push_room_message(&push_tx, &msg);

    sleep(Duration::from_millis(400)).await;

    assert!(rx_a.try_recv().is_ok(), "agent_a must receive broadcast message");
    assert!(rx_b.try_recv().is_ok(), "agent_b must receive broadcast message");
}
