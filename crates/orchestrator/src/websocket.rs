use crate::types::ToolPolicy;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// A handle to communicate with a connected agent's WebSocket.
#[derive(Debug, Clone)]
pub struct AgentConnection {
    /// Send messages to the agent (server → claude).
    pub tx: mpsc::UnboundedSender<String>,
}

/// Callback invoked when an agent produces a "result" message.
pub type ResultCallback = Arc<dyn Fn(Uuid, bool) + Send + Sync>;

/// Manages all active WebSocket connections from claude code instances.
#[derive(Clone)]
pub struct ConnectionRegistry {
    connections: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
    result_callbacks: Arc<RwLock<Vec<ResultCallback>>>,
    /// Per-agent tool policies (set during agent creation).
    policies: Arc<RwLock<HashMap<Uuid, ToolPolicy>>>,
    /// Broadcast channel for the multiplexed agent stream.
    stream_tx: broadcast::Sender<String>,
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        let (stream_tx, _) = broadcast::channel(256);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            result_callbacks: Arc::new(RwLock::new(Vec::new())),
            policies: Arc::new(RwLock::new(HashMap::new())),
            stream_tx,
        }
    }

    /// Subscribe to the multiplexed agent message stream.
    pub fn subscribe_stream(&self) -> broadcast::Receiver<String> {
        self.stream_tx.subscribe()
    }

    pub async fn register(&self, agent_id: Uuid, conn: AgentConnection) {
        self.connections.write().await.insert(agent_id, conn);
        info!(%agent_id, "Agent WebSocket registered");
    }

    pub async fn unregister(&self, agent_id: &Uuid) {
        self.connections.write().await.remove(agent_id);
        self.policies.write().await.remove(agent_id);
        info!(%agent_id, "Agent WebSocket unregistered");
    }

    /// Set the tool policy for an agent (called during agent creation).
    pub async fn set_policy(&self, agent_id: Uuid, policy: ToolPolicy) {
        self.policies.write().await.insert(agent_id, policy);
    }

    /// Get the tool policy for an agent (defaults to AllowAll if not set).
    pub async fn get_policy(&self, agent_id: &Uuid) -> ToolPolicy {
        self.policies.read().await.get(agent_id).cloned().unwrap_or_default()
    }

    /// Send a user message (prompt) to a connected agent.
    ///
    /// Uses the Claude Code SDK `stream-json` input format:
    /// `{"type": "user", "message": {"role": "user", "content": "..."}}`
    pub async fn send_user_message(&self, agent_id: &Uuid, content: &str) -> anyhow::Result<()> {
        let connections = self.connections.read().await;
        let conn = connections
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent {} not connected", agent_id))?;

        let msg = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": content,
            }
        });
        conn.tx
            .send(serde_json::to_string(&msg)? + "\n")
            .map_err(|e| anyhow::anyhow!("Failed to send to agent: {}", e))?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn is_connected(&self, agent_id: &Uuid) -> bool {
        self.connections.read().await.contains_key(agent_id)
    }

    pub async fn connected_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Register a callback to be invoked when any agent produces a "result" message.
    pub async fn on_result(&self, callback: ResultCallback) {
        self.result_callbacks.write().await.push(callback);
    }

    /// Notify all registered callbacks that an agent has completed a task.
    pub async fn notify_result(&self, agent_id: Uuid, is_error: bool) {
        let callbacks = self.result_callbacks.read().await;
        for cb in callbacks.iter() {
            cb(agent_id, is_error);
        }
    }
}

/// Axum handler for WebSocket upgrade at /ws/{agent_id}.
pub async fn ws_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    info!(%agent_id, "WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_agent_socket(socket, agent_id, registry))
}

async fn handle_agent_socket(socket: WebSocket, agent_id: Uuid, registry: ConnectionRegistry) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Channel for sending messages to this agent.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn = AgentConnection { tx };
    registry.register(agent_id, conn).await;

    // Task: forward messages from channel to WebSocket.
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Main loop: receive messages from claude code and handle protocol.
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                handle_incoming_message(&agent_id, &text, &registry).await;
            }
            Message::Ping(data) => {
                debug!(%agent_id, "Received ping");
                // Pong is handled automatically by axum's WebSocket impl.
                let _ = data; // consumed
            }
            Message::Close(_) => {
                info!(%agent_id, "WebSocket closed by client");
                break;
            }
            _ => {}
        }
    }

    // Cleanup.
    send_task.abort();
    registry.unregister(&agent_id).await;
    info!(%agent_id, "Agent WebSocket connection ended");
}

/// Process an incoming NDJSON message from a claude code instance.
async fn handle_incoming_message(agent_id: &Uuid, text: &str, registry: &ConnectionRegistry) {
    // Claude sends NDJSON — each line is a separate JSON message.
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                warn!(%agent_id, %e, "Failed to parse message from agent");
                continue;
            }
        };

        // Broadcast to the multiplexed stream (tagged with agent_id).
        let mut stream_msg = msg.clone();
        if let Some(obj) = stream_msg.as_object_mut() {
            obj.insert("agent_id".to_string(), Value::String(agent_id.to_string()));
        }
        let _ = registry.stream_tx.send(stream_msg.to_string());

        let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        debug!(%agent_id, %msg_type, "Received message from agent");

        match msg_type {
            "system" => {
                let subtype = msg.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                debug!(%agent_id, %subtype, "System message from agent");
            }
            "assistant" => {
                debug!(%agent_id, "Assistant response received");
            }
            "result" => {
                let is_error = msg.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                if is_error {
                    warn!(%agent_id, "Agent query completed with error");
                } else {
                    info!(%agent_id, "Agent query completed successfully");
                }
                registry.notify_result(*agent_id, is_error).await;
            }
            "control_request" => {
                handle_control_request(agent_id, &msg, registry).await;
            }
            "keep_alive" => {
                debug!(%agent_id, "Keep-alive from agent");
            }
            _ => {
                debug!(%agent_id, %msg_type, "Unhandled message type");
            }
        }
    }
}

/// Handle control requests from claude code (e.g., tool permission requests).
/// Evaluates tool requests against the agent's tool policy.
async fn handle_control_request(agent_id: &Uuid, msg: &Value, registry: &ConnectionRegistry) {
    let request_id = msg.get("request_id").and_then(|v| v.as_str()).unwrap_or("");

    let request = match msg.get("request") {
        Some(r) => r,
        None => return,
    };

    let subtype = request.get("subtype").and_then(|v| v.as_str()).unwrap_or("");

    match subtype {
        "can_use_tool" => {
            let tool_name = request.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let input = request.get("input").cloned().unwrap_or(Value::Object(Default::default()));

            let policy = registry.get_policy(agent_id).await;
            let allowed = policy.evaluate(tool_name);

            if allowed {
                info!(%agent_id, %tool_name, decision = "allow", "Tool use decision");

                let response = serde_json::json!({
                    "type": "control_response",
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": {
                            "behavior": "allow",
                            "updatedInput": input,
                        }
                    }
                });

                if let Err(e) = send_raw(agent_id, &response, registry).await {
                    error!(%agent_id, %e, "Failed to send control response");
                }
            } else {
                warn!(%agent_id, %tool_name, decision = "deny", "Tool use decision");

                let response = serde_json::json!({
                    "type": "control_response",
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": {
                            "behavior": "deny",
                            "message": format!("Tool '{}' is not allowed by agent policy", tool_name),
                        }
                    }
                });

                if let Err(e) = send_raw(agent_id, &response, registry).await {
                    error!(%agent_id, %e, "Failed to send deny response");
                }
            }
        }
        _ => {
            debug!(%agent_id, %subtype, "Unhandled control request subtype");
        }
    }
}

/// Axum handler for the multiplexed stream at /stream.
///
/// Clients receive all agent messages from all connected agents,
/// each tagged with an `agent_id` field.
pub async fn ws_stream_all_handler(
    ws: WebSocketUpgrade,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    info!("Stream (all) WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_stream_socket(socket, registry, None))
}

/// Axum handler for a per-agent stream at /stream/{agent_id}.
///
/// Clients receive only messages from the specified agent.
pub async fn ws_stream_agent_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    info!(%agent_id, "Stream (agent) WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_stream_socket(socket, registry, Some(agent_id)))
}

async fn handle_stream_socket(
    socket: WebSocket,
    registry: ConnectionRegistry,
    filter_agent: Option<Uuid>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut stream_rx = registry.subscribe_stream();

    let label = match filter_agent {
        Some(id) => format!("agent {}", id),
        None => "all".to_string(),
    };
    info!(filter = %label, "Stream client connected");

    // Task: forward broadcast messages to the stream client.
    let send_task = tokio::spawn(async move {
        loop {
            match stream_rx.recv().await {
                Ok(msg) => {
                    // If filtering by agent, parse and check agent_id.
                    if let Some(filter_id) = filter_agent {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg) {
                            let msg_agent = parsed
                                .get("agent_id")
                                .and_then(|v| v.as_str())
                                .and_then(|s| Uuid::parse_str(s).ok());
                            if msg_agent != Some(filter_id) {
                                continue;
                            }
                        }
                    }

                    if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Stream client lagged, skipped messages");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Drain incoming messages (stream clients don't send meaningful data).
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Close(_) => {
                info!(filter = %label, "Stream client disconnected");
                break;
            }
            Message::Ping(_) => {} // auto-pong by axum
            _ => {}
        }
    }

    send_task.abort();
    info!(filter = %label, "Stream WebSocket connection ended");
}

async fn send_raw(
    agent_id: &Uuid,
    msg: &Value,
    registry: &ConnectionRegistry,
) -> anyhow::Result<()> {
    let connections = registry.connections.read().await;
    let conn = connections
        .get(agent_id)
        .ok_or_else(|| anyhow::anyhow!("Agent {} not connected", agent_id))?;

    conn.tx
        .send(serde_json::to_string(msg)? + "\n")
        .map_err(|e| anyhow::anyhow!("Failed to send to agent: {}", e))?;

    Ok(())
}
