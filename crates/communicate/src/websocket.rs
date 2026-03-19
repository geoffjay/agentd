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

        // Remove from connections list
        if let Some(ref pid) = participant_id {
            if let Some(conns) = self.connections.write().await.get_mut(pid) {
                conns.retain(|(id, _)| *id != conn_id);
                if conns.is_empty() {
                    self.connections.write().await.remove(pid);
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
