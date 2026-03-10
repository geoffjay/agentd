use crate::approvals::ApprovalRegistry;
use crate::types::{ApprovalDecision, ResultInfo, ToolPolicy, UsageSnapshot};
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
pub type ResultCallback = Arc<dyn Fn(ResultInfo) + Send + Sync>;

/// Manages all active WebSocket connections from claude code instances.
#[derive(Clone)]
pub struct ConnectionRegistry {
    connections: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
    result_callbacks: Arc<RwLock<Vec<ResultCallback>>>,
    /// Per-agent tool policies (set during agent creation).
    policies: Arc<RwLock<HashMap<Uuid, ToolPolicy>>>,
    /// Broadcast channel for the multiplexed agent stream.
    stream_tx: broadcast::Sender<String>,
    /// In-memory store of pending human tool approvals.
    pub approvals: ApprovalRegistry,
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
            approvals: ApprovalRegistry::new(300), // 5-minute default timeout
        }
    }

    /// Broadcast a raw JSON string to all /stream subscribers.
    pub fn broadcast(&self, msg: String) {
        let _ = self.stream_tx.send(msg);
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
    pub async fn notify_result(&self, info: ResultInfo) {
        let callbacks = self.result_callbacks.read().await;
        for cb in callbacks.iter() {
            cb(info.clone());
        }
    }
}

/// Axum handler for WebSocket upgrade at /ws/{agent_id}.
///
/// This endpoint is reserved for agent CLI processes. Only one connection per
/// agent is allowed — a second connection would replace the first, severing
/// communication with the real agent. Use /stream/{agent_id} for read-only
/// monitoring.
pub async fn ws_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    if registry.is_connected(&agent_id).await {
        warn!(%agent_id, "Rejected WebSocket upgrade: agent already connected. Use /stream/{agent_id} for monitoring.");
        return axum::http::StatusCode::CONFLICT.into_response();
    }
    info!(%agent_id, "WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_agent_socket(socket, agent_id, registry)).into_response()
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

/// Extract usage data from a Claude Code `result` message.
///
/// Token counts are read from the nested `usage` object. Top-level fields
/// (`total_cost_usd`, `num_turns`, `duration_ms`, `duration_api_ms`) are read
/// from the message root, falling back to the `usage` sub-object for
/// backwards compatibility.
///
/// Returns `None` when the `usage` block is absent entirely.  Individual
/// missing fields within the block default to `0` (or `0.0` for cost).
fn extract_usage(msg: &Value) -> Option<UsageSnapshot> {
    let usage = msg.get("usage")?;

    Some(UsageSnapshot {
        input_tokens: usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        output_tokens: usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        total_cost_usd: msg
            .get("total_cost_usd")
            .or_else(|| usage.get("total_cost_usd"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        num_turns: msg
            .get("num_turns")
            .or_else(|| usage.get("num_turns"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        duration_ms: msg
            .get("duration_ms")
            .or_else(|| usage.get("duration_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        duration_api_ms: msg
            .get("duration_api_ms")
            .or_else(|| usage.get("duration_api_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
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
                let usage = extract_usage(&msg);
                registry.notify_result(ResultInfo { agent_id: *agent_id, is_error, usage }).await;
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
    let request_id = msg.get("request_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let request = match msg.get("request") {
        Some(r) => r,
        None => return,
    };

    let subtype = request.get("subtype").and_then(|v| v.as_str()).unwrap_or("");

    match subtype {
        "can_use_tool" => {
            let tool_name =
                request.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let input = request.get("input").cloned().unwrap_or(Value::Object(Default::default()));

            let policy = registry.get_policy(agent_id).await;
            let policy_mode = policy.mode_str();

            match policy {
                ToolPolicy::RequireApproval => {
                    // Spawn a separate task that holds the response until a human decides.
                    // The recv loop continues immediately so keep_alive etc. are processed.
                    info!(%agent_id, %tool_name, %policy_mode, "Tool use requires human approval, holding...");
                    let registry = registry.clone();
                    let agent_id = *agent_id;
                    tokio::spawn(async move {
                        handle_approval_hold(agent_id, request_id, tool_name, input, registry)
                            .await;
                    });
                }
                _ => {
                    let allowed = policy.evaluate(&tool_name);
                    if allowed {
                        info!(%agent_id, %tool_name, decision = "allow", %policy_mode, "Tool use decision");
                        let response = make_allow_response(&request_id, &input);
                        if let Err(e) = send_raw(agent_id, &response, registry).await {
                            error!(%agent_id, %e, "Failed to send control response");
                        }
                    } else {
                        warn!(%agent_id, %tool_name, decision = "deny", %policy_mode, "Tool use decision");
                        let response = make_deny_response(
                            &request_id,
                            &tool_name,
                            "not allowed by agent policy",
                        );
                        if let Err(e) = send_raw(agent_id, &response, registry).await {
                            error!(%agent_id, %e, "Failed to send deny response");
                        }
                    }
                }
            }
        }
        _ => {
            debug!(%agent_id, %subtype, "Unhandled control request subtype");
        }
    }
}

/// Default approval timeout in seconds (5 minutes).
const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// Hold a tool request pending human approval.
///
/// Registers the request in the ApprovalRegistry, broadcasts a `pending_approval`
/// event on the stream, then waits for a decision (or timeout). Sends the
/// appropriate control_response to the agent when resolved.
async fn handle_approval_hold(
    agent_id: Uuid,
    request_id: String,
    tool_name: String,
    tool_input: Value,
    registry: ConnectionRegistry,
) {
    let (approval, rx) = registry
        .approvals
        .register(agent_id, request_id.clone(), tool_name.clone(), tool_input.clone())
        .await;

    // Broadcast pending_approval event for stream subscribers / UIs
    let stream_event = serde_json::json!({
        "type": "pending_approval",
        "agent_id": agent_id,
        "approval_id": approval.id,
        "tool_name": tool_name,
        "tool_input": tool_input,
        "expires_at": approval.expires_at,
    });
    registry.broadcast(stream_event.to_string());

    // Wait for human decision or timeout
    let timeout = tokio::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS);
    let decision = tokio::time::timeout(timeout, rx).await;

    match decision {
        Ok(Ok(ApprovalDecision::Approve)) => {
            info!(%agent_id, %tool_name, approval_id = %approval.id, "Tool approved by human");
            let response = make_allow_response(&request_id, &tool_input);
            if let Err(e) = send_raw(&agent_id, &response, &registry).await {
                error!(%agent_id, %e, "Failed to send approve response");
            }
        }
        Ok(Ok(ApprovalDecision::Deny)) | Ok(Err(_)) => {
            warn!(%agent_id, %tool_name, approval_id = %approval.id, "Tool denied by human");
            let response = make_deny_response(&request_id, &tool_name, "denied by human operator");
            if let Err(e) = send_raw(&agent_id, &response, &registry).await {
                error!(%agent_id, %e, "Failed to send deny response");
            }
        }
        Err(_elapsed) => {
            warn!(%agent_id, %tool_name, approval_id = %approval.id, "Approval timed out, auto-denying");
            registry.approvals.mark_timed_out(&approval.id).await;
            let response = make_deny_response(
                &request_id,
                &tool_name,
                "approval timeout — no human decision within 5 minutes",
            );
            if let Err(e) = send_raw(&agent_id, &response, &registry).await {
                error!(%agent_id, %e, "Failed to send timeout-deny response");
            }
        }
    }
}

fn make_allow_response(request_id: &str, input: &Value) -> Value {
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": { "behavior": "allow", "updatedInput": input }
        }
    })
}

fn make_deny_response(request_id: &str, tool_name: &str, reason: &str) -> Value {
    serde_json::json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": {
                "behavior": "deny",
                "message": format!("Tool '{}': {}", tool_name, reason),
            }
        }
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_usage_full() {
        let msg = json!({
            "type": "result",
            "is_error": false,
            "usage": {
                "input_tokens": 1500,
                "output_tokens": 800,
                "cache_read_input_tokens": 200,
                "cache_creation_input_tokens": 50
            },
            "total_cost_usd": 0.0123,
            "num_turns": 3,
            "duration_ms": 5000,
            "duration_api_ms": 4200
        });

        let usage = extract_usage(&msg).expect("should extract usage");
        assert_eq!(usage.input_tokens, 1500);
        assert_eq!(usage.output_tokens, 800);
        assert_eq!(usage.cache_read_input_tokens, 200);
        assert_eq!(usage.cache_creation_input_tokens, 50);
        assert!((usage.total_cost_usd - 0.0123).abs() < 1e-9);
        assert_eq!(usage.num_turns, 3);
        assert_eq!(usage.duration_ms, 5000);
        assert_eq!(usage.duration_api_ms, 4200);
    }

    #[test]
    fn test_extract_usage_missing_block_returns_none() {
        let msg = json!({
            "type": "result",
            "is_error": false,
            "total_cost_usd": 0.01
        });

        assert!(extract_usage(&msg).is_none());
    }

    #[test]
    fn test_extract_usage_partial_fields_default_to_zero() {
        // Only input_tokens present in the usage block; everything else defaults.
        let msg = json!({
            "type": "result",
            "usage": {
                "input_tokens": 42
            }
        });

        let usage = extract_usage(&msg).expect("should extract usage");
        assert_eq!(usage.input_tokens, 42);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert!((usage.total_cost_usd - 0.0).abs() < 1e-9);
        assert_eq!(usage.num_turns, 0);
        assert_eq!(usage.duration_ms, 0);
        assert_eq!(usage.duration_api_ms, 0);
    }

    #[test]
    fn test_extract_usage_empty_usage_object() {
        let msg = json!({
            "type": "result",
            "usage": {}
        });

        let usage = extract_usage(&msg).expect("should extract usage from empty block");
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert!((usage.total_cost_usd - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_usage_top_level_fields_preferred() {
        // When both top-level and nested fields exist, top-level wins.
        let msg = json!({
            "type": "result",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "total_cost_usd": 0.001,
                "num_turns": 1,
                "duration_ms": 100,
                "duration_api_ms": 80,
            },
            "total_cost_usd": 0.999,
            "num_turns": 99,
            "duration_ms": 9999,
            "duration_api_ms": 8888,
        });

        let usage = extract_usage(&msg).expect("should extract usage");
        // Top-level fields should take precedence.
        assert!((usage.total_cost_usd - 0.999).abs() < 1e-9);
        assert_eq!(usage.num_turns, 99);
        assert_eq!(usage.duration_ms, 9999);
        assert_eq!(usage.duration_api_ms, 8888);
        // Token fields always come from the nested usage object.
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn test_extract_usage_fallback_to_nested_for_top_level_fields() {
        // When top-level fields are absent, fall back to the usage sub-object.
        let msg = json!({
            "type": "result",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "total_cost_usd": 0.005,
                "num_turns": 2,
                "duration_ms": 300,
                "duration_api_ms": 250,
            }
        });

        let usage = extract_usage(&msg).expect("should extract usage");
        assert!((usage.total_cost_usd - 0.005).abs() < 1e-9);
        assert_eq!(usage.num_turns, 2);
        assert_eq!(usage.duration_ms, 300);
        assert_eq!(usage.duration_api_ms, 250);
    }
}
