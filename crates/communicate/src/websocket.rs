//! WebSocket support for real-time message streaming.
//!
//! This module provides a `ConnectionManager` that tracks active WebSocket
//! connections and broadcasts room events (messages, participant join/leave)
//! to subscribed clients.

use crate::storage::CommunicateStorage;
use crate::types::{MessageResponse, ParticipantResponse};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use agentd_common::error::ApiError;

/// Unique identifier for a WebSocket connection.
type ConnectionId = Uuid;

/// A connection entry containing the connection ID and message sender.
type ConnectionEntry = (ConnectionId, mpsc::UnboundedSender<ServerMessage>);

/// Events that can occur in a room and should be broadcast to subscribers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoomEvent {
    Message(MessageResponse),
    ParticipantJoined(ParticipantResponse),
    ParticipantLeft { room_id: Uuid, identifier: String },
}

/// Manages active WebSocket connections and room subscriptions.
#[derive(Clone)]
pub struct ConnectionManager {
    /// Maps participant identifier to their active connection senders.
    connections: Arc<RwLock<HashMap<String, Vec<ConnectionEntry>>>>,
    /// Maps room ID to broadcast channel for that room.
    room_broadcasts: Arc<RwLock<HashMap<Uuid, broadcast::Sender<RoomEvent>>>>,
    /// Maps connection ID to the set of rooms they're subscribed to.
    subscriptions: Arc<RwLock<HashMap<ConnectionId, HashSet<Uuid>>>>,
    /// Maps connection ID to participant identifier.
    connection_participants: Arc<RwLock<HashMap<ConnectionId, String>>>,
}

impl ConnectionManager {
    /// Creates a new connection manager.
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            room_broadcasts: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            connection_participants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new WebSocket connection.
    pub async fn connect(
        &self,
        participant_id: String,
        sender: mpsc::UnboundedSender<ServerMessage>,
    ) -> ConnectionId {
        let conn_id = Uuid::new_v4();

        self.connections
            .write()
            .await
            .entry(participant_id.clone())
            .or_insert_with(Vec::new)
            .push((conn_id, sender));

        self.connection_participants.write().await.insert(conn_id, participant_id.clone());

        info!(%conn_id, %participant_id, "WebSocket connection registered");
        conn_id
    }

    /// Subscribes a connection to a room's events.
    ///
    /// Verifies that the participant is actually in the room before allowing subscription.
    pub async fn subscribe(
        &self,
        conn_id: ConnectionId,
        room_id: Uuid,
        storage: &CommunicateStorage,
    ) -> Result<broadcast::Receiver<RoomEvent>, ApiError> {
        // Get the participant identifier for this connection
        let participant_id = {
            let conn_participants = self.connection_participants.read().await;
            conn_participants.get(&conn_id).cloned().ok_or(ApiError::NotFound)?
        };

        // Verify participant is in the room
        let participant = storage
            .get_participant(&room_id, &participant_id)
            .await
            .map_err(|_| ApiError::NotFound)?;

        if participant.is_none() {
            return Err(ApiError::Forbidden("You are not a participant in this room".to_string()));
        }

        // Get or create the broadcast channel for this room
        let sender = {
            let mut room_broadcasts = self.room_broadcasts.write().await;
            room_broadcasts.entry(room_id).or_insert_with(|| broadcast::channel(1024).0).clone()
        };

        // Record the subscription
        self.subscriptions
            .write()
            .await
            .entry(conn_id)
            .or_insert_with(HashSet::new)
            .insert(room_id);

        info!(%conn_id, %room_id, %participant_id, "Subscribed to room");
        Ok(sender.subscribe())
    }

    /// Unsubscribes a connection from a room's events.
    pub async fn unsubscribe(&self, conn_id: ConnectionId, room_id: Uuid) {
        if let Some(subs) = self.subscriptions.write().await.get_mut(&conn_id) {
            subs.remove(&room_id);
            info!(%conn_id, %room_id, "Unsubscribed from room");
        }
    }

    /// Disconnects and cleans up a WebSocket connection.
    pub async fn disconnect(&self, conn_id: ConnectionId) {
        // Get participant ID before removing
        let participant_id = self.connection_participants.write().await.remove(&conn_id);

        // Remove subscriptions
        self.subscriptions.write().await.remove(&conn_id);

        // Remove from connections list.
        // Important: hold a single write-guard for both the retain and the
        // potential remove so we never try to re-acquire the same lock while
        // the guard is still alive (which would deadlock on tokio's RwLock).
        if let Some(ref pid) = participant_id {
            let mut conns_guard = self.connections.write().await;
            if let Some(conn_list) = conns_guard.get_mut(pid) {
                conn_list.retain(|(id, _)| *id != conn_id);
                if conn_list.is_empty() {
                    conns_guard.remove(pid);
                }
            }
        }

        info!(%conn_id, participant_id = ?participant_id, "WebSocket connection disconnected");
    }

    /// Broadcasts an event to all subscribers of a room.
    pub async fn broadcast_to_room(&self, room_id: Uuid, event: RoomEvent) {
        let room_broadcasts = self.room_broadcasts.read().await;
        if let Some(sender) = room_broadcasts.get(&room_id) {
            // broadcast::send returns Err if there are no receivers, which is fine
            let _ = sender.send(event);
        }
    }

    /// Gets the broadcast sender for a room, creating one if it doesn't exist.
    #[allow(dead_code)]
    pub async fn get_room_sender(&self, room_id: Uuid) -> broadcast::Sender<RoomEvent> {
        let mut room_broadcasts = self.room_broadcasts.write().await;
        room_broadcasts.entry(room_id).or_insert_with(|| broadcast::channel(1024).0).clone()
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WebSocket Protocol Messages
// ---------------------------------------------------------------------------

/// Parameters for establishing a WebSocket connection.
#[derive(Debug, Deserialize)]
pub struct WsConnectParams {
    /// Unique identifier for the participant (e.g., agent ID or username).
    pub identifier: String,
    /// Type of participant: "agent" or "human".
    pub kind: String,
    /// Human-readable display name.
    pub display_name: String,
}

/// Messages sent from the client to the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    /// Subscribe to receive events from a room.
    Subscribe { room_id: Uuid },
    /// Unsubscribe from a room's events.
    Unsubscribe { room_id: Uuid },
    /// Send a message to a room.
    Send {
        room_id: Uuid,
        content: String,
        #[serde(default)]
        metadata: HashMap<String, String>,
    },
    /// Ping to keep connection alive.
    Ping,
}

/// Messages sent from the server to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerMessage {
    /// A new message in a subscribed room.
    Message { room_id: Uuid, message: MessageResponse },
    /// A participant joined or left a room.
    ParticipantEvent {
        room_id: Uuid,
        event: String, // "joined" or "left"
        participant: Option<ParticipantResponse>,
        identifier: Option<String>, // For left events
    },
    /// An error occurred.
    Error { message: String },
    /// Pong response to ping.
    Pong,
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<crate::api::ApiState>,
    Query(params): Query<WsConnectParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate participant kind
    if params.kind != "agent" && params.kind != "human" {
        return Err(ApiError::InvalidInput("kind must be 'agent' or 'human'".to_string()));
    }

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, params)))
}

/// Handles an individual WebSocket connection.
#[allow(dead_code)]
async fn handle_socket(socket: WebSocket, state: crate::api::ApiState, params: WsConnectParams) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Create a channel for sending messages to this connection
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Register the connection
    let conn_id = state.connection_manager.connect(params.identifier.clone(), tx.clone()).await;

    // Spawn a task to forward messages from our channel to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                }
            }
        }
    });

    // Track active subscription tasks
    let mut subscription_tasks: HashMap<Uuid, tokio::task::JoinHandle<()>> = HashMap::new();

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Subscribe { room_id }) => {
                        debug!(%conn_id, %room_id, "Subscribe request");

                        match state
                            .connection_manager
                            .subscribe(conn_id, room_id, &state.storage)
                            .await
                        {
                            Ok(mut room_rx) => {
                                // Spawn a task to forward room events to this connection
                                let tx_clone = tx.clone();
                                let task = tokio::spawn(async move {
                                    while let Ok(event) = room_rx.recv().await {
                                        let server_msg = match event {
                                            RoomEvent::Message(msg) => {
                                                ServerMessage::Message { room_id, message: msg }
                                            }
                                            RoomEvent::ParticipantJoined(p) => {
                                                ServerMessage::ParticipantEvent {
                                                    room_id,
                                                    event: "joined".to_string(),
                                                    participant: Some(p),
                                                    identifier: None,
                                                }
                                            }
                                            RoomEvent::ParticipantLeft {
                                                room_id: _,
                                                identifier,
                                            } => ServerMessage::ParticipantEvent {
                                                room_id,
                                                event: "left".to_string(),
                                                participant: None,
                                                identifier: Some(identifier),
                                            },
                                        };

                                        if tx_clone.send(server_msg).is_err() {
                                            break;
                                        }
                                    }
                                });

                                subscription_tasks.insert(room_id, task);
                            }
                            Err(e) => {
                                warn!(%conn_id, %room_id, error = %e, "Subscribe failed");
                                let _ = tx.send(ServerMessage::Error { message: e.to_string() });
                            }
                        }
                    }
                    Ok(ClientMessage::Unsubscribe { room_id }) => {
                        debug!(%conn_id, %room_id, "Unsubscribe request");
                        state.connection_manager.unsubscribe(conn_id, room_id).await;

                        // Cancel the subscription task
                        if let Some(task) = subscription_tasks.remove(&room_id) {
                            task.abort();
                        }
                    }
                    Ok(ClientMessage::Send { room_id, content, metadata }) => {
                        debug!(%conn_id, %room_id, "Send message request");

                        // Use the storage layer to send the message
                        use crate::types::{CreateMessageRequest, ParticipantKind};
                        let req = CreateMessageRequest {
                            sender_id: params.identifier.clone(),
                            sender_name: params.display_name.clone(),
                            sender_kind: if params.kind == "agent" {
                                ParticipantKind::Agent
                            } else {
                                ParticipantKind::Human
                            },
                            content,
                            metadata,
                            reply_to: None,
                        };

                        match state.storage.send_message(&room_id, &req).await {
                            Ok(msg) => {
                                // Convert to response
                                let msg_response = MessageResponse::from(msg);

                                // Broadcast to all subscribers
                                state
                                    .connection_manager
                                    .broadcast_to_room(room_id, RoomEvent::Message(msg_response))
                                    .await;
                            }
                            Err(e) => {
                                warn!(%conn_id, %room_id, error = %e, "Send message failed");
                                let _ = tx.send(ServerMessage::Error {
                                    message: format!("Failed to send message: {}", e),
                                });
                            }
                        }
                    }
                    Ok(ClientMessage::Ping) => {
                        let _ = tx.send(ServerMessage::Pong);
                    }
                    Err(e) => {
                        warn!(%conn_id, error = %e, "Invalid message format");
                        let _ = tx.send(ServerMessage::Error {
                            message: format!("Invalid message format: {}", e),
                        });
                    }
                }
            }
            Ok(Message::Close(_)) => {
                debug!(%conn_id, "Client closed connection");
                break;
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                // Axum handles ping/pong automatically
            }
            Ok(Message::Binary(_)) => {
                warn!(%conn_id, "Unexpected binary message");
            }
            Err(e) => {
                error!(%conn_id, error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    send_task.abort();
    for (_, task) in subscription_tasks {
        task.abort();
    }
    state.connection_manager.disconnect(conn_id).await;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CommunicateStorage;
    use crate::types::{
        AddParticipantRequest, CreateRoomRequest, MessageResponse, MessageStatus, ParticipantKind,
        ParticipantRole, RoomType,
    };
    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    async fn create_test_storage() -> (CommunicateStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = CommunicateStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    fn make_add_participant_req(identifier: &str) -> AddParticipantRequest {
        AddParticipantRequest {
            identifier: identifier.to_string(),
            kind: ParticipantKind::Agent,
            display_name: format!("{identifier} Display"),
            role: ParticipantRole::Member,
        }
    }

    async fn create_room_with_participant(
        storage: &CommunicateStorage,
        room_name: &str,
        participant_id: &str,
    ) -> Uuid {
        let room = storage
            .create_room(&CreateRoomRequest {
                name: room_name.to_string(),
                topic: None,
                description: None,
                room_type: RoomType::Group,
                created_by: "creator".to_string(),
            })
            .await
            .unwrap();
        storage.add_participant(&room.id, &make_add_participant_req(participant_id)).await.unwrap();
        room.id
    }

    // -----------------------------------------------------------------------
    // Protocol message serde tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_client_message_subscribe_deserialization() {
        let json = r#"{"type":"subscribe","room_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Subscribe { room_id } => {
                assert_eq!(room_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected Subscribe, got {:?}", msg),
        }
    }

    #[test]
    fn test_client_message_unsubscribe_deserialization() {
        let json = r#"{"type":"unsubscribe","room_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Unsubscribe { .. }));
    }

    #[test]
    fn test_client_message_send_deserialization() {
        let json = r#"{"type":"send","room_id":"550e8400-e29b-41d4-a716-446655440000","content":"Hello!"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Send { content, metadata, .. } => {
                assert_eq!(content, "Hello!");
                assert!(metadata.is_empty()); // default empty
            }
            _ => panic!("Expected Send, got {:?}", msg),
        }
    }

    #[test]
    fn test_client_message_send_with_metadata_deserialization() {
        let json = r#"{"type":"send","room_id":"550e8400-e29b-41d4-a716-446655440000","content":"Hi","metadata":{"key":"value"}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Send { metadata, .. } => {
                assert_eq!(metadata.get("key").map(|s| s.as_str()), Some("value"));
            }
            _ => panic!("Expected Send, got {:?}", msg),
        }
    }

    #[test]
    fn test_client_message_ping_deserialization() {
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
    }

    #[test]
    fn test_client_message_invalid_type_returns_error() {
        let json = r#"{"type":"invalid_type"}"#;
        let result = serde_json::from_str::<ClientMessage>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_message_pong_serialization() {
        let msg = ServerMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "pong");
    }

    #[test]
    fn test_server_message_error_serialization() {
        let msg = ServerMessage::Error { message: "something went wrong".to_string() };
        let json = serde_json::to_string(&msg).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "error");
        assert_eq!(val["message"], "something went wrong");
    }

    #[test]
    fn test_server_message_participant_event_joined_serialization() {
        let msg = ServerMessage::ParticipantEvent {
            room_id: Uuid::nil(),
            event: "joined".to_string(),
            participant: None,
            identifier: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "participant_event");
        assert_eq!(val["event"], "joined");
    }

    #[test]
    fn test_server_message_message_serialization() {
        let room_id = Uuid::new_v4();
        let msg_response = MessageResponse {
            id: Uuid::new_v4(),
            room_id,
            sender_id: "agent-1".to_string(),
            sender_name: "Agent One".to_string(),
            sender_kind: ParticipantKind::Agent,
            content: "Hello, world!".to_string(),
            metadata: HashMap::new(),
            reply_to: None,
            status: MessageStatus::Sent,
            created_at: Utc::now(),
        };
        let msg = ServerMessage::Message { room_id, message: msg_response };
        let json = serde_json::to_string(&msg).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["type"], "message");
        assert_eq!(val["message"]["content"], "Hello, world!");
    }

    // -----------------------------------------------------------------------
    // ConnectionManager tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_connect_registers_connection() {
        let manager = ConnectionManager::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;
        assert!(!conn_id.is_nil());
    }

    #[tokio::test]
    async fn test_connect_multiple_connections_same_participant_get_unique_ids() {
        let manager = ConnectionManager::new();
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();
        let conn_id1 = manager.connect("agent-1".to_string(), tx1).await;
        let conn_id2 = manager.connect("agent-1".to_string(), tx2).await;
        assert_ne!(conn_id1, conn_id2);
    }

    #[tokio::test]
    async fn test_disconnect_removes_connection_participant_mapping() {
        let manager = ConnectionManager::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;

        manager.disconnect(conn_id).await;

        let conn_participants = manager.connection_participants.read().await;
        assert!(!conn_participants.contains_key(&conn_id));
    }

    #[tokio::test]
    async fn test_subscribe_unknown_connection_returns_not_found() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();
        let unknown_conn_id = Uuid::new_v4();

        let result = manager.subscribe(unknown_conn_id, Uuid::new_v4(), &storage).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_subscribe_participant_not_in_room_returns_forbidden() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        // Create room but don't add the agent as a participant.
        let room = storage
            .create_room(&CreateRoomRequest {
                name: "test-room".to_string(),
                topic: None,
                description: None,
                room_type: RoomType::Group,
                created_by: "creator".to_string(),
            })
            .await
            .unwrap();

        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("non-participant".to_string(), tx).await;

        let result = manager.subscribe(conn_id, room.id, &storage).await;
        assert!(result.is_err(), "subscribe should fail when agent is not a room participant");
    }

    #[tokio::test]
    async fn test_subscribe_valid_participant_returns_receiver() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        let room_id = create_room_with_participant(&storage, "test-room", "agent-1").await;

        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;

        let result = manager.subscribe(conn_id, room_id, &storage).await;
        assert!(result.is_ok(), "subscribe should succeed for a valid participant");
    }

    #[tokio::test]
    async fn test_unsubscribe_removes_room_from_subscriptions() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        let room_id = create_room_with_participant(&storage, "test-room", "agent-1").await;

        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;
        manager.subscribe(conn_id, room_id, &storage).await.unwrap();

        manager.unsubscribe(conn_id, room_id).await;

        let subs = manager.subscriptions.read().await;
        if let Some(rooms) = subs.get(&conn_id) {
            assert!(!rooms.contains(&room_id));
        }
    }

    #[tokio::test]
    async fn test_broadcast_to_room_delivers_event_to_subscriber() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        let room_id = create_room_with_participant(&storage, "test-room", "agent-1").await;

        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;
        let mut rx = manager.subscribe(conn_id, room_id, &storage).await.unwrap();

        manager
            .broadcast_to_room(
                room_id,
                RoomEvent::ParticipantLeft { room_id, identifier: "agent-2".to_string() },
            )
            .await;

        let event = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel was closed unexpectedly");

        match event {
            RoomEvent::ParticipantLeft { identifier, .. } => {
                assert_eq!(identifier, "agent-2");
            }
            _ => panic!("Expected ParticipantLeft, got {:?}", event),
        }
    }

    #[tokio::test]
    async fn test_broadcast_to_room_with_no_subscribers_does_not_panic() {
        let manager = ConnectionManager::new();
        let room_id = Uuid::new_v4();
        // No subscribers registered — broadcast should be a no-op, not a panic.
        manager
            .broadcast_to_room(
                room_id,
                RoomEvent::ParticipantLeft { room_id, identifier: "agent-1".to_string() },
            )
            .await;
    }

    #[tokio::test]
    async fn test_broadcast_fan_out_delivers_to_all_subscribers() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        let room = storage
            .create_room(&CreateRoomRequest {
                name: "fanout-room".to_string(),
                topic: None,
                description: None,
                room_type: RoomType::Group,
                created_by: "creator".to_string(),
            })
            .await
            .unwrap();

        // Add two participants and connect both.
        for agent in ["agent-a", "agent-b"] {
            storage.add_participant(&room.id, &make_add_participant_req(agent)).await.unwrap();
        }

        let (tx1, _rx1) = mpsc::unbounded_channel();
        let conn_id1 = manager.connect("agent-a".to_string(), tx1).await;
        let mut sub_rx1 = manager.subscribe(conn_id1, room.id, &storage).await.unwrap();

        let (tx2, _rx2) = mpsc::unbounded_channel();
        let conn_id2 = manager.connect("agent-b".to_string(), tx2).await;
        let mut sub_rx2 = manager.subscribe(conn_id2, room.id, &storage).await.unwrap();

        manager
            .broadcast_to_room(
                room.id,
                RoomEvent::ParticipantLeft { room_id: room.id, identifier: "agent-c".to_string() },
            )
            .await;

        let timeout = std::time::Duration::from_millis(200);
        let event1 = tokio::time::timeout(timeout, sub_rx1.recv())
            .await
            .expect("sub1 timed out")
            .expect("sub1 channel closed");
        let event2 = tokio::time::timeout(timeout, sub_rx2.recv())
            .await
            .expect("sub2 timed out")
            .expect("sub2 channel closed");

        assert!(matches!(event1, RoomEvent::ParticipantLeft { .. }));
        assert!(matches!(event2, RoomEvent::ParticipantLeft { .. }));
    }

    #[tokio::test]
    async fn test_disconnect_removes_all_subscriptions() {
        let (storage, _temp) = create_test_storage().await;
        let manager = ConnectionManager::new();

        let room_id = create_room_with_participant(&storage, "test-room", "agent-1").await;

        let (tx, _rx) = mpsc::unbounded_channel();
        let conn_id = manager.connect("agent-1".to_string(), tx).await;
        manager.subscribe(conn_id, room_id, &storage).await.unwrap();

        // Disconnect should clean up both subscriptions and the participant mapping.
        manager.disconnect(conn_id).await;

        let subs = manager.subscriptions.read().await;
        assert!(!subs.contains_key(&conn_id), "subscriptions should be removed on disconnect");

        let conn_participants = manager.connection_participants.read().await;
        assert!(
            !conn_participants.contains_key(&conn_id),
            "connection_participants should be removed on disconnect"
        );
    }

    #[tokio::test]
    async fn test_get_room_sender_creates_broadcast_channel() {
        let manager = ConnectionManager::new();
        let room_id = Uuid::new_v4();
        let sender = manager.get_room_sender(room_id).await;

        // Creating a receiver and sending should work without panic.
        let mut rx = sender.subscribe();
        let event = RoomEvent::ParticipantLeft { room_id, identifier: "x".to_string() };
        sender.send(event).unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        assert!(matches!(received, RoomEvent::ParticipantLeft { .. }));
    }
}
