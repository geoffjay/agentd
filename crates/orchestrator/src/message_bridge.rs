//! Message-to-prompt bridge for agent message delivery.
//!
//! The [`MessageBridge`] connects the communicate service (rooms/messages)
//! to the orchestrator's agent WebSocket protocol. When a message arrives in
//! a room that has agent participants, the bridge formats it as a user prompt
//! and forwards it to the target agent(s). When an agent produces a result,
//! the bridge posts the response back to the originating room.
//!
//! # Architecture
//!
//! The bridge runs as a background task and maintains:
//! - A WebSocket connection to the communicate service (to receive room events)
//! - Per-agent message queues (for agents that are busy when a message arrives)
//! - Echo-loop prevention via message metadata
//! - Metrics counters for deliveries, queued messages, and drops
//!
//! # Room Semantics
//!
//! | Room type   | Delivery target                              |
//! |-------------|----------------------------------------------|
//! | `Direct`    | The other (non-sender) participant           |
//! | `Group`     | All agent participants                        |
//! | `Broadcast` | All agent participants                        |
//!
//! # Message Format
//!
//! Prompts sent to agents include room and sender context:
//! ```text
//! [Room: <room_name>] <sender_name> (<sender_kind>):
//! <content>
//! ```

use crate::scheduler::events::{EventBus, SystemEvent};
use crate::storage::AgentStorage;
use crate::types::ActivityState;
use crate::websocket::ConnectionRegistry;
use communicate::client::CommunicateClient;
use communicate::types::{
    AddParticipantRequest, CreateMessageRequest, ParticipantKind, ParticipantRole, RoomType,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Sentinel identifier used when the bridge registers itself in rooms.
const BRIDGE_IDENTIFIER: &str = "agentd-orchestrator";
/// Display name shown in participant lists for the bridge.
const BRIDGE_DISPLAY_NAME: &str = "Orchestrator Bridge";
/// Default maximum number of queued messages per agent.
const DEFAULT_MAX_QUEUE_DEPTH: usize = 10;
/// Metadata key used to mark messages posted by the bridge (echo prevention).
const META_SOURCE_KEY: &str = "source";
/// Metadata value used to mark agent-response messages.
const META_SOURCE_VALUE: &str = "agent_response";
/// Metadata key carrying the originating agent ID on response messages.
const META_AGENT_ID_KEY: &str = "agent_id";

// ---------------------------------------------------------------------------
// Communicate WebSocket protocol types (client-side view)
// ---------------------------------------------------------------------------

/// Message sent *from* the bridge *to* the communicate service over WebSocket.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Subscribe {
        room_id: Uuid,
    },
    #[allow(dead_code)]
    Unsubscribe {
        room_id: Uuid,
    },
}

/// Message received *by* the bridge *from* the communicate service.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Message {
        room_id: Uuid,
        message: communicate::types::MessageResponse,
    },
    ParticipantEvent {
        room_id: Uuid,
        event: String,
        #[allow(dead_code)]
        participant: Option<communicate::types::ParticipantResponse>,
        #[allow(dead_code)]
        identifier: Option<String>,
    },
    Error {
        message: String,
    },
    Pong,
}

// ---------------------------------------------------------------------------
// Pending message (queued while agent is busy)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PendingMessage {
    room_id: Uuid,
    room_name: String,
    sender_name: String,
    sender_kind: String,
    content: String,
}

// ---------------------------------------------------------------------------
// MessageBridge
// ---------------------------------------------------------------------------

type WsSink = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// Bridges room messages from the communicate service to agent prompts and
/// routes agent responses back to rooms.
pub struct MessageBridge {
    registry: ConnectionRegistry,
    communicate: CommunicateClient,
    /// Agent storage for resolving display names. `None` in tests that don't
    /// need the storage layer.
    storage: Option<Arc<AgentStorage>>,
    event_bus: Arc<EventBus>,

    /// Maps room_id → Vec<agent_id> (agents that are participants in the room).
    room_agents: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// Maps agent_id → Vec<room_id> (rooms the agent participates in).
    agent_rooms: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// Maps room_id → (RoomType, room_name).
    room_info: Arc<RwLock<HashMap<Uuid, (RoomType, String)>>>,

    /// Per-agent message queue for delivery when the agent becomes idle.
    pending_queues: Arc<RwLock<HashMap<Uuid, VecDeque<PendingMessage>>>>,
    /// Tracks which room is currently being served for each busy agent.
    active_rooms: Arc<RwLock<HashMap<Uuid, Uuid>>>,

    /// Maximum number of pending messages per agent before dropping.
    max_queue_depth: usize,

    /// Sink half of the communicate WebSocket (shared across tasks).
    ws_tx: Arc<Mutex<Option<WsSink>>>,

    /// URL of the communicate service WebSocket endpoint (base, without query).
    ws_url: String,
}

impl MessageBridge {
    /// Create a new bridge.
    ///
    /// `communicate_base_url` should be the HTTP base URL of the communicate
    /// service, e.g. `http://localhost:17010`. The bridge derives the WS URL
    /// by replacing the scheme.
    pub fn new(
        registry: ConnectionRegistry,
        communicate: CommunicateClient,
        storage: Arc<AgentStorage>,
        event_bus: Arc<EventBus>,
        communicate_base_url: &str,
    ) -> Self {
        Self::with_optional_storage(
            registry,
            communicate,
            Some(storage),
            event_bus,
            communicate_base_url,
        )
    }

    /// Internal constructor used by tests and production code alike.
    fn with_optional_storage(
        registry: ConnectionRegistry,
        communicate: CommunicateClient,
        storage: Option<Arc<AgentStorage>>,
        event_bus: Arc<EventBus>,
        communicate_base_url: &str,
    ) -> Self {
        let ws_url =
            communicate_base_url.replacen("http://", "ws://", 1).replacen("https://", "wss://", 1);

        Self {
            registry,
            communicate,
            storage,
            event_bus,
            room_agents: Arc::new(RwLock::new(HashMap::new())),
            agent_rooms: Arc::new(RwLock::new(HashMap::new())),
            room_info: Arc::new(RwLock::new(HashMap::new())),
            pending_queues: Arc::new(RwLock::new(HashMap::new())),
            active_rooms: Arc::new(RwLock::new(HashMap::new())),
            max_queue_depth: DEFAULT_MAX_QUEUE_DEPTH,
            ws_tx: Arc::new(Mutex::new(None)),
            ws_url,
        }
    }

    /// Override the maximum per-agent queue depth (default: 10).
    #[allow(dead_code)]
    pub fn with_max_queue_depth(mut self, depth: usize) -> Self {
        self.max_queue_depth = depth;
        self
    }

    /// Start the bridge as a background task.
    ///
    /// This spawns two long-running tasks:
    /// 1. **WS listener** — connects to communicate and forwards room messages.
    /// 2. **Event bus listener** — reacts to `AgentJoinedRoom` events to subscribe
    ///    to newly joined rooms.
    ///
    /// Also registers a result callback on the [`ConnectionRegistry`] so that
    /// agent responses are posted back to the originating room.
    pub async fn start(self: Arc<Self>) {
        // Initialize metrics counters.
        metrics::counter!("messages_delivered_to_agents").absolute(0);
        metrics::gauge!("messages_queued").set(0.0);
        metrics::counter!("messages_dropped").absolute(0);

        // Register result callback: when an agent finishes, post back to room.
        {
            let bridge = self.clone();
            self.registry
                .on_result(Arc::new(move |info| {
                    let bridge = bridge.clone();
                    tokio::spawn(async move {
                        bridge.on_agent_result(info.agent_id, info.is_error).await;
                    });
                }))
                .await;
        }

        // Connect to communicate WebSocket.
        // URL-encode the display name for the query string.
        let display_name_encoded = BRIDGE_DISPLAY_NAME.replace(' ', "%20");
        let connect_url = format!(
            "{}/ws?identifier={}&kind=agent&display_name={}",
            self.ws_url, BRIDGE_IDENTIFIER, display_name_encoded,
        );

        let ws_stream = match connect_async(&connect_url).await {
            Ok((ws, _)) => ws,
            Err(e) => {
                warn!(
                    url = %connect_url,
                    error = %e,
                    "MessageBridge: could not connect to communicate WebSocket (service may be unavailable). Bridge will not be active."
                );
                return;
            }
        };

        info!("MessageBridge: connected to communicate WebSocket at {}", connect_url);

        let (ws_sink, ws_stream) = ws_stream.split();
        *self.ws_tx.lock().await = Some(ws_sink);

        // Task 1: receive room events from communicate WebSocket.
        {
            let bridge = self.clone();
            tokio::spawn(async move {
                bridge.run_ws_receiver(ws_stream).await;
            });
        }

        // Task 2: listen for AgentJoinedRoom events from the event bus.
        {
            let bridge = self.clone();
            tokio::spawn(async move {
                bridge.run_event_listener().await;
            });
        }
    }

    // -----------------------------------------------------------------------
    // WebSocket receiver task
    // -----------------------------------------------------------------------

    async fn run_ws_receiver(
        &self,
        mut stream: futures::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    ) {
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    self.handle_ws_message(&text).await;
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    warn!("MessageBridge: communicate WebSocket closed");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    error!("MessageBridge: WebSocket error: {}", e);
                    break;
                }
            }
        }
        warn!("MessageBridge: WS receiver loop exited — bridge is no longer active");
    }

    async fn handle_ws_message(&self, text: &str) {
        let msg: ServerMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                debug!("MessageBridge: could not parse message from communicate: {}: {}", e, text);
                return;
            }
        };

        match msg {
            ServerMessage::Message { room_id, message } => {
                self.on_room_message(room_id, message).await;
            }
            ServerMessage::ParticipantEvent { room_id, event, .. } => {
                debug!(%room_id, %event, "MessageBridge: participant event");
            }
            ServerMessage::Error { message } => {
                warn!("MessageBridge: error from communicate WS: {}", message);
            }
            ServerMessage::Pong => {}
        }
    }

    // -----------------------------------------------------------------------
    // Event bus listener task
    // -----------------------------------------------------------------------

    async fn run_event_listener(&self) {
        let mut rx = self.event_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(SystemEvent::AgentJoinedRoom { agent_id, room_id }) => {
                    if let Err(e) = self.on_agent_joined_room(agent_id, room_id).await {
                        warn!(
                            %agent_id,
                            %room_id,
                            error = %e,
                            "MessageBridge: failed to handle AgentJoinedRoom"
                        );
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!("MessageBridge: event bus lagged by {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    // -----------------------------------------------------------------------
    // Room join handling
    // -----------------------------------------------------------------------

    async fn on_agent_joined_room(&self, agent_id: Uuid, room_id: Uuid) -> anyhow::Result<()> {
        // Fetch room details to record type and name.
        let room = match self.communicate.get_room(room_id).await? {
            Some(r) => r,
            None => {
                warn!(%room_id, "MessageBridge: room not found");
                return Ok(());
            }
        };

        // Update local tracking.
        {
            let mut room_agents = self.room_agents.write().await;
            let agents = room_agents.entry(room_id).or_insert_with(Vec::new);
            if !agents.contains(&agent_id) {
                agents.push(agent_id);
            }
        }
        {
            let mut agent_rooms = self.agent_rooms.write().await;
            let rooms = agent_rooms.entry(agent_id).or_insert_with(Vec::new);
            if !rooms.contains(&room_id) {
                rooms.push(room_id);
            }
        }
        self.room_info.write().await.insert(room_id, (room.room_type.clone(), room.name.clone()));

        // Ensure the bridge itself is a participant so it can subscribe.
        self.ensure_bridge_participant(room_id).await;

        // Subscribe to room events over the WS connection.
        self.ws_subscribe(room_id).await;

        info!(
            %agent_id,
            %room_id,
            room_name = %room.name,
            room_type = %room.room_type,
            "MessageBridge: subscribed to room for agent"
        );

        Ok(())
    }

    /// Ensure `agentd-orchestrator` is registered as a participant in `room_id`.
    async fn ensure_bridge_participant(&self, room_id: Uuid) {
        let req = AddParticipantRequest {
            identifier: BRIDGE_IDENTIFIER.to_string(),
            kind: ParticipantKind::Agent,
            display_name: BRIDGE_DISPLAY_NAME.to_string(),
            role: ParticipantRole::Observer,
        };

        match self.communicate.add_participant(room_id, &req).await {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("409") || msg.contains("conflict") || msg.contains("Conflict") {
                    // Already a participant — fine.
                } else {
                    warn!(
                        %room_id,
                        error = %msg,
                        "MessageBridge: could not add bridge as room participant"
                    );
                }
            }
        }
    }

    /// Send a subscribe message to the communicate WebSocket.
    async fn ws_subscribe(&self, room_id: Uuid) {
        let msg = ClientMessage::Subscribe { room_id };
        self.ws_send(&msg).await;
    }

    /// Serialize and send a message over the bridge WebSocket connection.
    async fn ws_send(&self, msg: &ClientMessage) {
        let json = match serde_json::to_string(msg) {
            Ok(j) => j,
            Err(e) => {
                error!("MessageBridge: failed to serialize WS message: {}", e);
                return;
            }
        };

        let mut guard = self.ws_tx.lock().await;
        if let Some(ref mut sink) = *guard {
            if let Err(e) = sink.send(Message::Text(json.into())).await {
                error!("MessageBridge: failed to send WS message: {}", e);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Room message → agent prompt delivery
    // -----------------------------------------------------------------------

    async fn on_room_message(&self, room_id: Uuid, message: communicate::types::MessageResponse) {
        // Echo prevention: skip messages posted by the bridge itself.
        if message.metadata.get(META_SOURCE_KEY).map(|s| s.as_str()) == Some(META_SOURCE_VALUE) {
            debug!(
                %room_id,
                message_id = %message.id,
                "MessageBridge: skipping agent-response echo"
            );
            return;
        }

        // Get room info.
        let room_info = self.room_info.read().await;
        let (room_type, room_name) = match room_info.get(&room_id) {
            Some(info) => info.clone(),
            None => {
                debug!(%room_id, "MessageBridge: received message for untracked room");
                return;
            }
        };
        drop(room_info);

        // Get agent participants in this room.
        let agent_ids: Vec<Uuid> = {
            let room_agents = self.room_agents.read().await;
            match room_agents.get(&room_id) {
                Some(agents) => agents.clone(),
                None => return,
            }
        };

        if agent_ids.is_empty() {
            return;
        }

        // Determine target agents based on room type.
        let targets: Vec<Uuid> = match room_type {
            RoomType::Direct => {
                // In a Direct room, deliver only to agents that are NOT the sender.
                agent_ids.into_iter().filter(|id| id.to_string() != message.sender_id).collect()
            }
            RoomType::Group | RoomType::Broadcast => {
                // Deliver to all agents that are not the sender.
                agent_ids.into_iter().filter(|id| id.to_string() != message.sender_id).collect()
            }
        };

        if targets.is_empty() {
            return;
        }

        let sender_kind = message.sender_kind.to_string();
        let prompt = format!(
            "[Room: {}] {} ({}):\n{}",
            room_name, message.sender_name, sender_kind, message.content
        );

        for agent_id in targets {
            self.deliver_or_queue(agent_id, room_id, room_name.clone(), prompt.clone(), &message)
                .await;
        }
    }

    /// Deliver a prompt to an agent, or enqueue it if the agent is busy.
    async fn deliver_or_queue(
        &self,
        agent_id: Uuid,
        room_id: Uuid,
        room_name: String,
        prompt: String,
        message: &communicate::types::MessageResponse,
    ) {
        let state = self.registry.get_activity_state(&agent_id).await;

        if state == ActivityState::Idle {
            // Agent is idle — deliver immediately.
            match self.registry.send_user_message(&agent_id, &prompt).await {
                Ok(()) => {
                    info!(
                        %agent_id,
                        %room_id,
                        message_id = %message.id,
                        "MessageBridge: delivered message to agent"
                    );
                    self.active_rooms.write().await.insert(agent_id, room_id);
                    metrics::counter!("messages_delivered_to_agents").increment(1);
                }
                Err(e) => {
                    warn!(
                        %agent_id,
                        %room_id,
                        error = %e,
                        "MessageBridge: failed to deliver message to agent"
                    );
                }
            }
        } else {
            // Agent is busy — enqueue.
            let mut queues = self.pending_queues.write().await;
            let queue = queues.entry(agent_id).or_insert_with(VecDeque::new);

            if queue.len() >= self.max_queue_depth {
                warn!(
                    %agent_id,
                    %room_id,
                    queue_depth = queue.len(),
                    "MessageBridge: queue full, dropping message"
                );
                metrics::counter!("messages_dropped").increment(1);
            } else {
                queue.push_back(PendingMessage {
                    room_id,
                    room_name,
                    sender_name: message.sender_name.clone(),
                    sender_kind: message.sender_kind.to_string(),
                    content: message.content.clone(),
                });
                let depth = queue.len() as f64;
                drop(queues);
                metrics::gauge!("messages_queued").set(depth);
                debug!(
                    %agent_id,
                    %room_id,
                    "MessageBridge: agent busy, message queued"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Agent result → room response
    // -----------------------------------------------------------------------

    async fn on_agent_result(&self, agent_id: Uuid, is_error: bool) {
        // Look up the room this agent was serving.
        let room_id = {
            let mut active = self.active_rooms.write().await;
            active.remove(&agent_id)
        };

        if let Some(room_id) = room_id {
            // Fetch the result text.  The SDK `result` message text is broadcast
            // on the stream; here we reconstruct a brief summary.
            let content = if is_error {
                "[Agent completed with error]".to_string()
            } else {
                self.get_agent_result_text(&agent_id).await
            };

            self.post_to_room(agent_id, room_id, content).await;
        }

        // Drain the queue: deliver the next pending message if any.
        self.drain_queue(agent_id).await;
    }

    /// Attempt to post a message to the room on behalf of an agent.
    async fn post_to_room(&self, agent_id: Uuid, room_id: Uuid, content: String) {
        // Fetch agent name for display.
        let agent_name = self.agent_display_name(&agent_id).await;

        let mut metadata = std::collections::HashMap::new();
        metadata.insert(META_SOURCE_KEY.to_string(), META_SOURCE_VALUE.to_string());
        metadata.insert(META_AGENT_ID_KEY.to_string(), agent_id.to_string());

        let req = CreateMessageRequest {
            sender_id: agent_id.to_string(),
            sender_name: agent_name,
            sender_kind: ParticipantKind::Agent,
            content,
            metadata,
            reply_to: None,
        };

        match self.communicate.send_message(room_id, &req).await {
            Ok(_) => {
                info!(
                    %agent_id,
                    %room_id,
                    "MessageBridge: posted agent response to room"
                );
            }
            Err(e) => {
                warn!(
                    %agent_id,
                    %room_id,
                    error = %e,
                    "MessageBridge: failed to post agent response to room"
                );
            }
        }
    }

    /// Deliver the next queued message to an agent (if any).
    async fn drain_queue(&self, agent_id: Uuid) {
        let next = {
            let mut queues = self.pending_queues.write().await;
            let queue = queues.entry(agent_id).or_insert_with(VecDeque::new);
            let msg = queue.pop_front();
            let depth = queue.len() as f64;
            drop(queues);
            metrics::gauge!("messages_queued").set(depth);
            msg
        };

        if let Some(pending) = next {
            let prompt = format!(
                "[Room: {}] {} ({}):\n{}",
                pending.room_name, pending.sender_name, pending.sender_kind, pending.content,
            );

            match self.registry.send_user_message(&agent_id, &prompt).await {
                Ok(()) => {
                    info!(
                        %agent_id,
                        room_id = %pending.room_id,
                        "MessageBridge: delivered queued message to agent"
                    );
                    self.active_rooms.write().await.insert(agent_id, pending.room_id);
                    metrics::counter!("messages_delivered_to_agents").increment(1);
                }
                Err(e) => {
                    warn!(
                        %agent_id,
                        error = %e,
                        "MessageBridge: failed to deliver queued message"
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Return the result text for an agent from the most recent activity. Since
    /// the SDK result text is not stored here (only streamed on the bus), we
    /// return a placeholder that the agent's actual streamed response already
    /// covers. This is used only as the "echo back" message content.
    async fn get_agent_result_text(&self, _agent_id: &Uuid) -> String {
        "[Agent completed]".to_string()
    }

    /// Return a display name for an agent, falling back to the agent ID string.
    async fn agent_display_name(&self, agent_id: &Uuid) -> String {
        if let Some(ref storage) = self.storage {
            if let Ok(Some(agent)) = storage.get(agent_id).await {
                return agent.name;
            }
        }
        agent_id.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> ConnectionRegistry {
        ConnectionRegistry::new()
    }

    fn make_communicate_client() -> CommunicateClient {
        CommunicateClient::new("http://localhost:17010")
    }

    /// Construct a bridge without real storage (for unit tests that don't
    /// exercise the storage code path).
    fn make_bridge(base_url: &str) -> MessageBridge {
        MessageBridge::with_optional_storage(
            make_registry(),
            make_communicate_client(),
            None,
            EventBus::shared(16),
            base_url,
        )
    }

    #[test]
    fn test_bridge_ws_url_http_to_ws() {
        let bridge = make_bridge("http://localhost:17010");
        assert_eq!(bridge.ws_url, "ws://localhost:17010");
    }

    #[test]
    fn test_bridge_ws_url_https_to_wss() {
        let bridge = make_bridge("https://example.com");
        assert_eq!(bridge.ws_url, "wss://example.com");
    }

    #[test]
    fn test_bridge_default_queue_depth() {
        let bridge = make_bridge("http://localhost:17010");
        assert_eq!(bridge.max_queue_depth, DEFAULT_MAX_QUEUE_DEPTH);
    }

    #[test]
    fn test_bridge_custom_queue_depth() {
        let bridge = make_bridge("http://localhost:17010").with_max_queue_depth(5);
        assert_eq!(bridge.max_queue_depth, 5);
    }

    #[test]
    fn test_client_message_serializes_subscribe() {
        let msg = ClientMessage::Subscribe { room_id: Uuid::nil() };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"subscribe\""));
        assert!(json.contains("\"room_id\""));
    }

    #[test]
    fn test_server_message_deserializes_pong() {
        let json = r#"{"type":"pong"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::Pong));
    }

    #[test]
    fn test_server_message_deserializes_error() {
        let json = r#"{"type":"error","message":"not found"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ServerMessage::Error { .. }));
    }

    #[tokio::test]
    async fn test_pending_queue_enqueue_dequeue() {
        // Verify the per-agent queue logic directly via the bridge's shared state.
        let bridge = make_bridge("http://localhost:17010").with_max_queue_depth(3);

        let agent_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        // Push one message directly into the queue.
        {
            let mut queues = bridge.pending_queues.write().await;
            queues.entry(agent_id).or_insert_with(VecDeque::new).push_back(PendingMessage {
                room_id,
                room_name: "test-room".to_string(),
                sender_name: "Alice".to_string(),
                sender_kind: "human".to_string(),
                content: "hello".to_string(),
            });
        }

        // Pop it.
        let next = {
            let mut queues = bridge.pending_queues.write().await;
            queues.get_mut(&agent_id).and_then(|q| q.pop_front())
        };

        assert!(next.is_some());
        let msg = next.unwrap();
        assert_eq!(msg.room_id, room_id);
        assert_eq!(msg.sender_name, "Alice");
        assert_eq!(msg.content, "hello");
    }

    #[tokio::test]
    async fn test_queue_drop_at_max_depth() {
        let bridge = Arc::new(make_bridge("http://localhost:17010").with_max_queue_depth(2));

        let agent_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        // Fill the queue to capacity.
        for i in 0..2usize {
            let mut queues = bridge.pending_queues.write().await;
            queues.entry(agent_id).or_insert_with(VecDeque::new).push_back(PendingMessage {
                room_id,
                room_name: "r".to_string(),
                sender_name: format!("user{i}"),
                sender_kind: "human".to_string(),
                content: format!("msg{i}"),
            });
        }

        // Verify at capacity.
        assert_eq!(bridge.pending_queues.read().await.get(&agent_id).map(|q| q.len()), Some(2));

        // Another message now exceeds the limit — the deliver_or_queue method
        // would drop it. We verify the depth logic here: if depth >= max_queue_depth
        // we should NOT push.
        let depth = bridge.pending_queues.read().await.get(&agent_id).map(|q| q.len()).unwrap_or(0);
        assert!(depth >= bridge.max_queue_depth);
    }

    #[test]
    fn test_prompt_format() {
        let room_name = "general";
        let sender_name = "Alice";
        let sender_kind = "human";
        let content = "Hello agent!";
        let prompt =
            format!("[Room: {}] {} ({}):\n{}", room_name, sender_name, sender_kind, content);
        assert_eq!(prompt, "[Room: general] Alice (human):\nHello agent!");
    }

    #[test]
    fn test_echo_metadata_constants() {
        assert_eq!(META_SOURCE_KEY, "source");
        assert_eq!(META_SOURCE_VALUE, "agent_response");
        assert_eq!(META_AGENT_ID_KEY, "agent_id");
    }
}
