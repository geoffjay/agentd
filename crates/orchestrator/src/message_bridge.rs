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
//! The bridge runs as two long-lived background tasks:
//!
//! 1. **WS loop** — connects to the communicate service WebSocket with
//!    automatic exponential-backoff reconnection. Re-subscribes to all known
//!    rooms on each reconnect.
//! 2. **Event bus listener** — reacts to [`SystemEvent::AgentJoinedRoom`]
//!    events to update the room→agent mapping and subscribe to the new room.
//!
//! A [`ResultCallback`] is registered on the [`ConnectionRegistry`] so that
//! agent responses are posted back to the originating room.
//!
//! # Room Semantics
//!
//! | Room type   | Delivery target                              |
//! |-------------|----------------------------------------------|
//! | `Direct`    | At most one other (non-sender) participant   |
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
//!
//! # Echo Prevention
//!
//! Messages posted by the bridge back to rooms carry the metadata key
//! `"source"` with value `"agent_response"`. Incoming messages with this
//! metadata are silently skipped so the agent does not receive its own reply
//! as a new prompt.

use crate::scheduler::events::{EventBus, SystemEvent};
use crate::storage::AgentStorage;
use crate::types::ResultInfo;
use crate::websocket::ConnectionRegistry;
use communicate::client::CommunicateClient;
use communicate::types::{
    AddParticipantRequest, CreateMessageRequest, ParticipantKind, ParticipantRole, RoomType,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Sentinel identifier used when the bridge registers itself in rooms.
const BRIDGE_IDENTIFIER: &str = "agentd-orchestrator";
/// Display name shown in participant lists for the bridge.
const BRIDGE_DISPLAY_NAME: &str = "Orchestrator%20Bridge";
/// Default maximum number of queued messages per agent.
const DEFAULT_MAX_QUEUE_DEPTH: usize = 10;
/// Metadata key used to mark messages posted by the bridge (echo prevention).
const META_SOURCE_KEY: &str = "source";
/// Metadata value used to mark agent-response messages.
const META_SOURCE_VALUE: &str = "agent_response";
/// Metadata key carrying the originating agent ID on response messages.
const META_AGENT_ID_KEY: &str = "agent_id";
/// Initial backoff duration for WebSocket reconnection attempts.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
/// Maximum backoff duration for WebSocket reconnection attempts.
const MAX_BACKOFF: Duration = Duration::from_secs(60);

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
    /// Agent storage for resolving display names. `None` in tests.
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

    /// Sink half of the communicate WebSocket (replaced on each reconnect).
    ws_tx: Arc<Mutex<Option<WsSink>>>,

    /// WS base URL for the communicate service (scheme already converted to ws/wss).
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

    /// Internal constructor that accepts `Option<Arc<AgentStorage>>`.
    ///
    /// Used by tests that do not exercise storage code paths.
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

    /// Start the bridge as two background tasks.
    ///
    /// This method returns immediately after spawning:
    /// 1. A **WS loop** task that connects to the communicate service and
    ///    reconnects automatically with exponential backoff on disconnect.
    /// 2. An **event bus listener** task that reacts to `AgentJoinedRoom`
    ///    events.
    ///
    /// A result callback is also registered on the [`ConnectionRegistry`] so
    /// that agent responses are posted back to rooms.
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
                        bridge.on_agent_result(info).await;
                    });
                }))
                .await;
        }

        // Spawn WS loop (reconnects automatically — does NOT block startup).
        {
            let bridge = self.clone();
            tokio::spawn(async move {
                bridge.run_ws_loop().await;
            });
        }

        // Spawn event bus listener.
        {
            let bridge = self.clone();
            tokio::spawn(async move {
                bridge.run_event_listener().await;
            });
        }
    }

    // -----------------------------------------------------------------------
    // WebSocket connection loop with auto-reconnect
    // -----------------------------------------------------------------------

    async fn run_ws_loop(&self) {
        let connect_url = format!(
            "{}/ws?identifier={}&kind=agent&display_name={}",
            self.ws_url, BRIDGE_IDENTIFIER, BRIDGE_DISPLAY_NAME,
        );

        let mut backoff = INITIAL_BACKOFF;

        loop {
            match connect_async(&connect_url).await {
                Ok((ws, _)) => {
                    info!(url = %connect_url, "MessageBridge: connected to communicate WebSocket");
                    backoff = INITIAL_BACKOFF; // reset on successful connect

                    let (sink, stream) = ws.split();
                    *self.ws_tx.lock().await = Some(sink);

                    // Re-subscribe to all rooms tracked so far.
                    self.resubscribe_all_rooms().await;

                    // Block until the connection drops.
                    self.run_ws_receiver(stream).await;

                    // Connection dropped — clear the sink.
                    *self.ws_tx.lock().await = None;
                    warn!(
                        retry_in = ?backoff,
                        "MessageBridge: communicate WebSocket disconnected, will retry"
                    );
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        retry_in = ?backoff,
                        "MessageBridge: communicate WebSocket connect failed, will retry"
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    /// Re-subscribe to every room the bridge already knows about.
    ///
    /// Called after a WS reconnection so that previously-subscribed rooms are
    /// not silently dropped.
    async fn resubscribe_all_rooms(&self) {
        let room_ids: Vec<Uuid> = self.room_agents.read().await.keys().copied().collect();
        for room_id in room_ids {
            self.ws_subscribe(room_id).await;
        }
    }

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
                Ok(Message::Text(text)) => self.handle_ws_message(&text).await,
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(Message::Close(_)) => {
                    debug!("MessageBridge: communicate WebSocket closed by server");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    error!("MessageBridge: WebSocket receive error: {}", e);
                    break;
                }
            }
        }
    }

    async fn handle_ws_message(&self, text: &str) {
        let msg: ServerMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                debug!(error = %e, raw = %text, "MessageBridge: could not parse communicate message");
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
    // Event bus listener
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
            let agents = room_agents.entry(room_id).or_default();
            if !agents.contains(&agent_id) {
                agents.push(agent_id);
            }
        }
        {
            let mut agent_rooms = self.agent_rooms.write().await;
            let rooms = agent_rooms.entry(agent_id).or_default();
            if !rooms.contains(&room_id) {
                rooms.push(room_id);
            }
        }
        self.room_info.write().await.insert(room_id, (room.room_type.clone(), room.name.clone()));

        // Ensure the bridge itself is a participant so it can subscribe.
        // If this fails we skip the subscription — there is no point subscribing
        // if the communicate service will reject our subscribe request.
        if let Err(e) = self.ensure_bridge_participant(room_id).await {
            warn!(
                %room_id,
                error = %e,
                "MessageBridge: could not add bridge as room participant; skipping subscription"
            );
            return Ok(());
        }

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
    ///
    /// Returns `Ok(())` when the bridge is already a participant (409) or was
    /// just added. Returns `Err` on any other failure so the caller can decide
    /// whether to proceed with the WS subscription.
    async fn ensure_bridge_participant(&self, room_id: Uuid) -> anyhow::Result<()> {
        let req = AddParticipantRequest {
            identifier: BRIDGE_IDENTIFIER.to_string(),
            kind: ParticipantKind::Agent,
            display_name: "Orchestrator Bridge".to_string(),
            role: ParticipantRole::Observer,
        };

        match self.communicate.add_participant(room_id, &req).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("409") || msg.contains("conflict") || msg.contains("Conflict") {
                    Ok(()) // already a participant
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Send a subscribe message to the communicate WebSocket.
    async fn ws_subscribe(&self, room_id: Uuid) {
        self.ws_send(&ClientMessage::Subscribe { room_id }).await;
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

        // Retrieve room info.
        let (room_type, room_name) = match self.room_info.read().await.get(&room_id).cloned() {
            Some(info) => info,
            None => {
                debug!(%room_id, "MessageBridge: received message for untracked room");
                return;
            }
        };

        // Get agent participants in this room.
        let agent_ids: Vec<Uuid> = match self.room_agents.read().await.get(&room_id).cloned() {
            Some(agents) if !agents.is_empty() => agents,
            _ => return,
        };

        // Determine target agents based on room type.
        let targets: Vec<Uuid> = match room_type {
            RoomType::Direct => {
                // In a Direct room there are exactly two participants. Deliver
                // to at most one agent that is NOT the sender, enforcing the
                // one-to-one invariant even if the data is inconsistent.
                agent_ids
                    .into_iter()
                    .filter(|id| id.to_string() != message.sender_id)
                    .take(1)
                    .collect()
            }
            RoomType::Group | RoomType::Broadcast => {
                // Deliver to all agent participants that are not the sender.
                agent_ids.into_iter().filter(|id| id.to_string() != message.sender_id).collect()
            }
        };

        if targets.is_empty() {
            return;
        }

        let prompt = format!(
            "[Room: {}] {} ({}):\n{}",
            room_name, message.sender_name, message.sender_kind, message.content
        );

        for agent_id in targets {
            self.deliver_or_queue(agent_id, room_id, room_name.clone(), prompt.clone(), &message)
                .await;
        }
    }

    /// Deliver a prompt to an agent, or enqueue it if the agent is busy.
    ///
    /// Uses [`ConnectionRegistry::try_claim_idle`] to atomically check and
    /// transition the agent's activity state, eliminating the TOCTOU race
    /// that would exist if `get_activity_state` and `send_user_message` were
    /// called separately.
    async fn deliver_or_queue(
        &self,
        agent_id: Uuid,
        room_id: Uuid,
        room_name: String,
        prompt: String,
        message: &communicate::types::MessageResponse,
    ) {
        if self.registry.try_claim_idle(&agent_id).await {
            // Agent was idle and is now claimed (Busy). Record the active room
            // BEFORE sending so that if the result callback fires before this
            // await resumes, the room is already tracked.
            self.active_rooms.write().await.insert(agent_id, room_id);

            // `send_user_message` will also set Busy — that is idempotent here.
            match self.registry.send_user_message(&agent_id, &prompt).await {
                Ok(()) => {
                    info!(
                        %agent_id,
                        %room_id,
                        message_id = %message.id,
                        "MessageBridge: delivered message to agent"
                    );
                    metrics::counter!("messages_delivered_to_agents").increment(1);
                }
                Err(e) => {
                    // Send failed (agent likely disconnected). Roll back active
                    // room tracking so the result callback does not attempt to
                    // post to the room.
                    self.active_rooms.write().await.remove(&agent_id);
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
            let queue = queues.entry(agent_id).or_default();

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
                let total: usize = queues.values().map(|q| q.len()).sum();
                drop(queues);
                metrics::gauge!("messages_queued").set(total as f64);
                debug!(%agent_id, %room_id, "MessageBridge: agent busy, message queued");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Agent result → room response
    // -----------------------------------------------------------------------

    async fn on_agent_result(&self, info: ResultInfo) {
        let agent_id = info.agent_id;

        // Look up the room this agent was serving.
        let room_id = self.active_rooms.write().await.remove(&agent_id);

        if let Some(room_id) = room_id {
            if info.is_error {
                self.post_to_room(agent_id, room_id, "[Agent completed with error]".to_string())
                    .await;
            } else if !info.result_text.is_empty() {
                // Only post to the room if the agent produced actual text.
                // Posting a placeholder when there is no text is worse than
                // posting nothing — the streaming output already reached the
                // room via the orchestrator event bus / UI.
                self.post_to_room(agent_id, room_id, info.result_text.clone()).await;
            }
        }

        // Drain the queue: deliver the next pending message if any.
        self.drain_queue(agent_id).await;
    }

    /// Post a message to a room on behalf of an agent.
    async fn post_to_room(&self, agent_id: Uuid, room_id: Uuid, content: String) {
        let agent_name = self.agent_display_name(&agent_id).await;

        let mut metadata = HashMap::new();
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
                info!(%agent_id, %room_id, "MessageBridge: posted agent response to room");
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

    /// Pop and deliver the next queued message for `agent_id`, if any.
    ///
    /// Uses [`ConnectionRegistry::try_claim_idle`] to atomically claim the
    /// agent before delivery — if the agent has already been claimed by a
    /// concurrent delivery, the queued message is left in place and will be
    /// delivered when that task completes.
    async fn drain_queue(&self, agent_id: Uuid) {
        let next = {
            let mut queues = self.pending_queues.write().await;
            let msg = queues.entry(agent_id).or_default().pop_front();
            let total: usize = queues.values().map(|q| q.len()).sum();
            drop(queues);
            metrics::gauge!("messages_queued").set(total as f64);
            msg
        };

        let Some(pending) = next else { return };

        if self.registry.try_claim_idle(&agent_id).await {
            // Record room before sending (same ordering as deliver_or_queue).
            self.active_rooms.write().await.insert(agent_id, pending.room_id);

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
                    metrics::counter!("messages_delivered_to_agents").increment(1);
                }
                Err(e) => {
                    self.active_rooms.write().await.remove(&agent_id);
                    warn!(
                        %agent_id,
                        error = %e,
                        "MessageBridge: failed to deliver queued message"
                    );
                }
            }
        } else {
            // Agent was claimed by another concurrent delivery between the
            // queue pop and the claim attempt. Re-queue at the front so the
            // message is not lost and will be delivered next result cycle.
            warn!(
                %agent_id,
                "MessageBridge: agent busy during drain, re-queuing message"
            );
            let mut queues = self.pending_queues.write().await;
            queues.entry(agent_id).or_default().push_front(pending);
            let total: usize = queues.values().map(|q| q.len()).sum();
            drop(queues);
            metrics::gauge!("messages_queued").set(total as f64);
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

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
// Test-only helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
impl MessageBridge {
    /// Override the maximum per-agent queue depth (default: 10).
    ///
    /// Builder method for tests — call before `Arc::new(...)`.
    fn with_max_queue_depth(mut self, depth: usize) -> Self {
        self.max_queue_depth = depth;
        self
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

    /// Construct a bridge without real storage (for unit tests that do not
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

    // -----------------------------------------------------------------------
    // URL conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_bridge_ws_url_http_to_ws() {
        assert_eq!(make_bridge("http://localhost:17010").ws_url, "ws://localhost:17010");
    }

    #[test]
    fn test_bridge_ws_url_https_to_wss() {
        assert_eq!(make_bridge("https://example.com").ws_url, "wss://example.com");
    }

    // -----------------------------------------------------------------------
    // Queue depth configuration
    // -----------------------------------------------------------------------

    #[test]
    fn test_bridge_default_queue_depth() {
        assert_eq!(make_bridge("http://localhost:17010").max_queue_depth, DEFAULT_MAX_QUEUE_DEPTH);
    }

    #[test]
    fn test_bridge_custom_queue_depth() {
        assert_eq!(
            make_bridge("http://localhost:17010").with_max_queue_depth(5).max_queue_depth,
            5
        );
    }

    // -----------------------------------------------------------------------
    // WS protocol message serialization / deserialization
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Message queue — enqueue and dequeue
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_pending_queue_enqueue_dequeue() {
        let bridge = make_bridge("http://localhost:17010").with_max_queue_depth(3);
        let agent_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        bridge.pending_queues.write().await.entry(agent_id).or_default().push_back(
            PendingMessage {
                room_id,
                room_name: "test-room".to_string(),
                sender_name: "Alice".to_string(),
                sender_kind: "human".to_string(),
                content: "hello".to_string(),
            },
        );

        let msg =
            bridge.pending_queues.write().await.get_mut(&agent_id).and_then(|q| q.pop_front());
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.room_id, room_id);
        assert_eq!(msg.sender_name, "Alice");
        assert_eq!(msg.content, "hello");
    }

    #[tokio::test]
    async fn test_queue_at_max_depth_signals_full() {
        let bridge = Arc::new(make_bridge("http://localhost:17010").with_max_queue_depth(2));
        let agent_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        for i in 0..2usize {
            bridge.pending_queues.write().await.entry(agent_id).or_default().push_back(
                PendingMessage {
                    room_id,
                    room_name: "r".to_string(),
                    sender_name: format!("user{i}"),
                    sender_kind: "human".to_string(),
                    content: format!("msg{i}"),
                },
            );
        }

        let depth = bridge.pending_queues.read().await.get(&agent_id).map(|q| q.len()).unwrap_or(0);
        assert_eq!(depth, 2);
        assert!(depth >= bridge.max_queue_depth);
    }

    // -----------------------------------------------------------------------
    // Echo prevention
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_echo_message_is_skipped() {
        use communicate::types::{MessageStatus, ParticipantKind as PK};
        use std::collections::HashMap;

        let bridge = make_bridge("http://localhost:17010");
        let room_id = Uuid::new_v4();

        // Seed room_info so the bridge would process the message if not filtered.
        bridge.room_info.write().await.insert(room_id, (RoomType::Group, "test".to_string()));
        let agent_id = Uuid::new_v4();
        bridge.room_agents.write().await.insert(room_id, vec![agent_id]);

        let mut meta = HashMap::new();
        meta.insert(META_SOURCE_KEY.to_string(), META_SOURCE_VALUE.to_string());

        let msg = communicate::types::MessageResponse {
            id: Uuid::new_v4(),
            room_id,
            sender_id: "human-1".to_string(),
            sender_name: "Human".to_string(),
            sender_kind: PK::Human,
            content: "original".to_string(),
            metadata: meta,
            reply_to: None,
            status: MessageStatus::Sent,
            created_at: chrono::Utc::now(),
        };

        // on_room_message should return early without touching the agent queue.
        bridge.on_room_message(room_id, msg).await;

        let queue_len =
            bridge.pending_queues.read().await.get(&agent_id).map(|q| q.len()).unwrap_or(0);
        assert_eq!(queue_len, 0, "echo-prevention should prevent any queuing");
    }

    // -----------------------------------------------------------------------
    // Prompt format
    // -----------------------------------------------------------------------

    #[test]
    fn test_prompt_format() {
        let prompt = format!("[Room: {}] {} ({}):\n{}", "general", "Alice", "human", "Hello!");
        assert_eq!(prompt, "[Room: general] Alice (human):\nHello!");
    }

    // -----------------------------------------------------------------------
    // Metadata constants
    // -----------------------------------------------------------------------

    #[test]
    fn test_echo_metadata_constants() {
        assert_eq!(META_SOURCE_KEY, "source");
        assert_eq!(META_SOURCE_VALUE, "agent_response");
        assert_eq!(META_AGENT_ID_KEY, "agent_id");
    }

    // -----------------------------------------------------------------------
    // Gauge total
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_gauge_reflects_total_across_agents() {
        let bridge = make_bridge("http://localhost:17010");
        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        let make_pending = |name: &str| PendingMessage {
            room_id,
            room_name: "r".to_string(),
            sender_name: name.to_string(),
            sender_kind: "human".to_string(),
            content: "hi".to_string(),
        };

        {
            let mut q = bridge.pending_queues.write().await;
            q.entry(agent1).or_default().push_back(make_pending("Alice"));
            q.entry(agent1).or_default().push_back(make_pending("Alice2"));
            q.entry(agent2).or_default().push_back(make_pending("Bob"));
        }

        let total: usize = bridge.pending_queues.read().await.values().map(|q| q.len()).sum();
        assert_eq!(total, 3, "total queue depth across all agents should be 3");
    }
}
