use crate::approvals::ApprovalRegistry;
use crate::manager::AgentManager;
use crate::scheduler::events::{EventBus, SystemEvent};
use crate::storage::AgentStorage;
use crate::types::{
    ActivityState, ApprovalDecision, ConversationEvent, ConversationEventType, ResultInfo,
    ToolPolicy, UsageSnapshot,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
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
    /// Per-agent activity state (idle or busy).
    activity_states: Arc<RwLock<HashMap<Uuid, ActivityState>>>,
    /// Broadcast channel for the multiplexed agent stream.
    stream_tx: broadcast::Sender<String>,
    /// Notifies waiters when any agent connects.
    connect_notify: Arc<tokio::sync::Notify>,
    /// In-memory store of pending human tool approvals.
    pub approvals: ApprovalRegistry,
    /// Optional event bus for publishing lifecycle events.
    event_bus: Option<Arc<EventBus>>,
    /// Optional storage backend for persisting conversation events.
    storage: Option<AgentStorage>,
    /// Per-agent session counter — incremented on every context clear.
    session_numbers: Arc<RwLock<HashMap<Uuid, i64>>>,
    /// Per-agent strictly monotonic event sequence number.
    ///
    /// Independent of [`session_numbers`] (which resets on context clear),
    /// `seq` only ever increases for a given agent. It is the dedupe key for
    /// the snapshot+live streaming protocol on `/v2/stream/{agent_id}`.
    /// Backed by a `std::sync::Mutex` because increments are non-blocking
    /// and we want to call [`next_seq`] from sync code paths.
    seq_counters: Arc<std::sync::Mutex<HashMap<Uuid, i64>>>,
    /// Agents that have sent the AAP `ready` handshake and can accept prompts.
    ready_agents: Arc<RwLock<std::collections::HashSet<Uuid>>>,
    /// Notifies waiters when any agent becomes ready.
    ready_notify: Arc<tokio::sync::Notify>,
    /// Capabilities advertised by each agent's adapter in its `ready` message.
    capabilities: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionRegistry {
    pub fn new() -> Self {
        // Capacity 1024 leaves headroom for the v2 stream's brief snapshot
        // buffer; v1 clients see no behavioural change beyond unknown
        // `"seq"` fields they already ignore.
        let (stream_tx, _) = broadcast::channel(1024);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            result_callbacks: Arc::new(RwLock::new(Vec::new())),
            policies: Arc::new(RwLock::new(HashMap::new())),
            activity_states: Arc::new(RwLock::new(HashMap::new())),
            stream_tx,
            connect_notify: Arc::new(tokio::sync::Notify::new()),
            approvals: ApprovalRegistry::new(300), // 5-minute default timeout
            event_bus: None,
            storage: None,
            session_numbers: Arc::new(RwLock::new(HashMap::new())),
            seq_counters: Arc::new(std::sync::Mutex::new(HashMap::new())),
            ready_agents: Arc::new(RwLock::new(std::collections::HashSet::new())),
            ready_notify: Arc::new(tokio::sync::Notify::new()),
            capabilities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Mark an agent as ready (its adapter sent the AAP `ready` handshake) and
    /// wake any waiters.
    pub async fn mark_ready(&self, agent_id: Uuid) {
        self.ready_agents.write().await.insert(agent_id);
        self.ready_notify.notify_waiters();
    }

    /// Record the capabilities an agent's adapter advertised.
    pub async fn set_capabilities(&self, agent_id: Uuid, caps: Vec<String>) {
        self.capabilities.write().await.insert(agent_id, caps);
    }

    /// Return the capabilities advertised by an agent's adapter, if known.
    #[allow(dead_code)] // Public accessor for capability introspection (API/diagnostics).
    pub async fn get_capabilities(&self, agent_id: &Uuid) -> Option<Vec<String>> {
        self.capabilities.read().await.get(agent_id).cloned()
    }

    /// Whether an agent has completed the AAP `ready` handshake.
    pub async fn is_ready(&self, agent_id: &Uuid) -> bool {
        self.ready_agents.read().await.contains(agent_id)
    }

    /// Wait until an agent has sent `ready`, or until the timeout expires.
    /// Returns `true` if the agent became ready.
    pub async fn wait_for_ready(&self, agent_id: &Uuid, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.is_ready(agent_id).await {
                return true;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            tokio::select! {
                _ = self.ready_notify.notified() => {}
                _ = tokio::time::sleep(remaining) => {
                    return self.is_ready(agent_id).await;
                }
            }
        }
    }

    /// Create a registry with an event bus for publishing lifecycle events.
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Attach a storage backend for fire-and-forget conversation event persistence.
    pub fn with_storage(mut self, storage: AgentStorage) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Assign and return the next per-agent monotonic sequence number.
    ///
    /// Cheap, non-async — held under a brief `std::sync::Mutex`. The counter
    /// is seeded from storage in [`register`] so that values stay
    /// strictly increasing across orchestrator restarts.
    pub(crate) fn next_seq(&self, agent_id: Uuid) -> i64 {
        let mut counters = self.seq_counters.lock().expect("seq counter mutex poisoned");
        let entry = counters.entry(agent_id).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Assign the next per-agent `seq` to `event`, persist it synchronously,
    /// and return the assigned seq. Callers thread the returned seq into the
    /// live broadcast frame so v2 stream consumers can dedupe across the
    /// snapshot → live boundary.
    ///
    /// `await`ing the persist before returning is what guarantees the
    /// snapshot+live protocol's correctness: a client subscribing the
    /// instant the caller broadcasts must, in its subsequent
    /// `MAX(seq)` query, see the persisted row. If persist were
    /// fire-and-forget, the broadcast would arrive on live channels that
    /// were subscribed *after* the broadcast, and the snapshot would miss
    /// the row — producing a gap.
    ///
    /// If no storage backend is configured this still returns a valid seq;
    /// the event simply isn't persisted (and will not appear in any snapshot).
    pub async fn record_and_seq(&self, mut event: ConversationEvent) -> i64 {
        event.seq = self.next_seq(event.agent_id);
        let seq = event.seq;
        if let Some(ref storage) = self.storage {
            if let Err(e) = storage.insert_conversation_event(&event).await {
                warn!(
                    agent_id = %event.agent_id,
                    event_type = %event.event_type,
                    error = %e,
                    "Failed to persist conversation event"
                );
            }
        }
        seq
    }

    /// Return the current in-memory session number for an agent (default 0).
    async fn get_session_number(&self, agent_id: &Uuid) -> i64 {
        *self.session_numbers.read().await.get(agent_id).unwrap_or(&0)
    }

    /// Update the session counter and persist a `context_cleared` event.
    ///
    /// Called by [`AgentManager::clear_context`] after it has advanced the
    /// storage session and computed the new session number.
    pub async fn persist_context_cleared(&self, agent_id: Uuid, new_session_number: i64) {
        // Keep the in-memory counter in sync with the storage session.
        self.session_numbers.write().await.insert(agent_id, new_session_number);

        let event = ConversationEvent::new(
            agent_id,
            ConversationEventType::ContextCleared,
            new_session_number,
            None,
            None,
        );
        let seq = self.record_and_seq(event).await;

        // Broadcast the context-cleared event on the multiplexed stream so
        // live UI subscribers are notified immediately.
        let stream_event = serde_json::json!({
            "type": "agent:context_cleared",
            "seq": seq,
            "agent_id": agent_id.to_string(),
            "agentId": agent_id.to_string(),
            "session_number": new_session_number,
            "timestamp": Utc::now().to_rfc3339(),
        });
        let _ = self.stream_tx.send(stream_event.to_string());
    }

    /// Return a reference to the event bus, if one was configured.
    pub fn event_bus(&self) -> Option<&Arc<EventBus>> {
        self.event_bus.as_ref()
    }

    /// Return a clone of the configured storage backend, if any.
    ///
    /// `AgentStorage` is internally `Arc`-backed so the clone is cheap; the
    /// v2 stream handler uses this to query historical events without
    /// borrowing the registry across `await` points.
    pub fn storage(&self) -> Option<AgentStorage> {
        self.storage.clone()
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
        // Seed the seq counter from storage BEFORE the connection accepts
        // events. The unique `(agent_id, seq)` index would reject any
        // collision, so we must know the previous high-water mark up front.
        // This is awaited synchronously (single SQL query) — unlike the
        // session_number lazy-init which is fire-and-forget.
        let max_seq = if let Some(ref storage) = self.storage {
            storage.get_max_conversation_seq(agent_id).await.unwrap_or_else(|e| {
                warn!(%agent_id, error = %e, "Failed to seed seq counter from storage; starting at 0");
                0
            })
        } else {
            0
        };
        self.seq_counters.lock().expect("seq counter mutex poisoned").insert(agent_id, max_seq);

        // A fresh connection must re-send the AAP `ready` handshake; clear any
        // stale readiness/capabilities from a prior session.
        self.ready_agents.write().await.remove(&agent_id);
        self.capabilities.write().await.remove(&agent_id);

        self.connections.write().await.insert(agent_id, conn);
        self.activity_states.write().await.insert(agent_id, ActivityState::Idle);
        self.connect_notify.notify_waiters();
        if let Some(bus) = &self.event_bus {
            bus.publish(SystemEvent::AgentConnected { agent_id });
        }
        let active = self.connections.read().await.len();
        metrics::gauge!("agents_active").set(active as f64);
        info!(%agent_id, "Agent WebSocket registered");

        // Initialize the in-memory session counter from storage (fire-and-forget).
        // Defaults to 0 immediately; corrected to MAX(session_number) from
        // conversation_events once the async lookup completes (typically within
        // a few ms). Reading from conversation_events directly avoids any
        // dependency on usage-session semantics.
        self.session_numbers.write().await.entry(agent_id).or_insert(0);
        if let Some(ref storage) = self.storage {
            let storage = storage.clone();
            let session_numbers = self.session_numbers.clone();
            tokio::spawn(async move {
                match storage.get_max_conversation_session_number(agent_id).await {
                    Ok(session) => {
                        // Only update if the storage value is greater than what
                        // is currently in memory. This prevents the async
                        // initialisation from clobbering a newer value that may
                        // have been written by `persist_context_cleared` between
                        // the time `register` was called and now.
                        let mut guard = session_numbers.write().await;
                        let current = guard.entry(agent_id).or_insert(0);
                        if session > *current {
                            *current = session;
                        }
                    }
                    Err(e) => {
                        warn!(
                            %agent_id, error = %e,
                            "Failed to initialize session number from storage; defaulting to 0"
                        );
                    }
                }
            });
        }
    }

    /// Wait until a specific agent connects, or until the timeout expires.
    ///
    /// Returns `true` if the agent connected, `false` on timeout.
    pub async fn wait_for_agent(&self, agent_id: &Uuid, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.is_connected(agent_id).await {
                return true;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            tokio::select! {
                _ = self.connect_notify.notified() => {
                    // An agent connected — loop to check if it's ours.
                }
                _ = tokio::time::sleep(remaining) => {
                    return self.is_connected(agent_id).await;
                }
            }
        }
    }

    pub async fn unregister(&self, agent_id: &Uuid) {
        self.connections.write().await.remove(agent_id);
        self.policies.write().await.remove(agent_id);
        self.activity_states.write().await.remove(agent_id);
        self.session_numbers.write().await.remove(agent_id);
        self.seq_counters.lock().expect("seq counter mutex poisoned").remove(agent_id);
        self.ready_agents.write().await.remove(agent_id);
        self.capabilities.write().await.remove(agent_id);
        if let Some(bus) = &self.event_bus {
            bus.publish(SystemEvent::AgentDisconnected { agent_id: *agent_id });
        }
        let active = self.connections.read().await.len();
        metrics::gauge!("agents_active").set(active as f64);
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

    /// Get the current activity state for an agent.
    ///
    /// Returns `Idle` for agents that are not connected (no entry in the map).
    pub async fn get_activity_state(&self, agent_id: &Uuid) -> ActivityState {
        self.activity_states.read().await.get(agent_id).cloned().unwrap_or_default()
    }

    /// Atomically check whether an agent is `Idle` and, if so, transition it
    /// to `Busy`.
    ///
    /// Returns `true` when the agent was idle and has been claimed (it is now
    /// `Busy`).  Returns `false` when the agent was already busy or not
    /// connected — the caller should enqueue the message instead of delivering
    /// it.
    ///
    /// This eliminates the TOCTOU race that would exist if callers first called
    /// [`get_activity_state`] and then [`send_user_message`] (which sets `Busy`
    /// separately): a second concurrent caller can observe `Idle` between those
    /// two operations, causing two simultaneous deliveries.
    pub async fn try_claim_idle(&self, agent_id: &Uuid) -> bool {
        let mut states = self.activity_states.write().await;
        match states.get(agent_id) {
            Some(ActivityState::Idle) => {
                states.insert(*agent_id, ActivityState::Busy);
                true
            }
            _ => false,
        }
    }

    /// Send a user message (prompt) to a connected agent.
    ///
    /// Emits the AAP `prompt` message with a fresh `turn_id`. The adapter
    /// translates it into its agent's native format.
    ///
    /// Transitions the agent's activity state to `Busy`, broadcasts an
    /// `agent:activity_changed` event, and persists both a `prompt_sent` and
    /// an `activity_changed` conversation event.
    pub async fn send_user_message(&self, agent_id: &Uuid, content: &str) -> anyhow::Result<()> {
        use agentd_agent_protocol::{HostMessage, PromptContent};

        let connections = self.connections.read().await;
        let conn = connections
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent {} not connected", agent_id))?;

        let prompt = HostMessage::Prompt {
            turn_id: Uuid::new_v4().to_string(),
            content: PromptContent::Text(content.to_string()),
        };
        conn.tx
            .send(prompt.to_ndjson()? + "\n")
            .map_err(|e| anyhow::anyhow!("Failed to send to agent: {}", e))?;

        drop(connections);

        // Transition to Busy. Assign seq numbers for both events first so that
        // the broadcast frames carry the same seq the snapshot replay will see.
        self.activity_states.write().await.insert(*agent_id, ActivityState::Busy);
        let session = self.get_session_number(agent_id).await;

        let prompt_seq = self
            .record_and_seq(ConversationEvent::new(
                *agent_id,
                ConversationEventType::PromptSent,
                session,
                Some(content.to_string()),
                None,
            ))
            .await;
        let prompt_event = serde_json::json!({
            "type": "agent:prompt_sent",
            "seq": prompt_seq,
            "agent_id": agent_id.to_string(),
            "agentId": agent_id.to_string(),
            "line": content,
            "session_number": session,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self.stream_tx.send(prompt_event.to_string());

        let activity_seq = self
            .record_and_seq(ConversationEvent::new(
                *agent_id,
                ConversationEventType::ActivityChanged,
                session,
                Some("busy".to_string()),
                None,
            ))
            .await;
        let activity_event = serde_json::json!({
            "type": "agent:activity_changed",
            "seq": activity_seq,
            "agent_id": agent_id.to_string(),
            "agentId": agent_id.to_string(),
            "activity": "busy",
            "session_number": session,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self.stream_tx.send(activity_event.to_string());

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn is_connected(&self, agent_id: &Uuid) -> bool {
        self.connections.read().await.contains_key(agent_id)
    }

    pub async fn connected_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Return the set of currently connected agent IDs.
    pub async fn connected_ids(&self) -> Vec<Uuid> {
        self.connections.read().await.keys().copied().collect()
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
/// Query parameters for the agent WebSocket endpoint.
#[derive(serde::Deserialize, Default)]
pub struct WsAgentParams {
    /// Optional bearer token — validated against the core service when
    /// `AGENTD_CORE_SERVICE_URL` is set.
    pub token: Option<String>,
}

pub async fn ws_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    axum::extract::Query(params): axum::extract::Query<WsAgentParams>,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    // Validate token if provided and core service URL is configured.
    if let Some(ref token) = params.token {
        if let Some(result) = agentd_common::ws_auth::validate_ws_token(token).await {
            if result.is_err() {
                return axum::http::StatusCode::UNAUTHORIZED.into_response();
            }
        }
    }

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

    // AAP handshake: the adapter (running in a tmux/docker session, dialing
    // back over this websocket) needs the `initialize` message carrying all
    // agent configuration before it will accept prompts. Queue it before the
    // connection is registered so it is the first frame the adapter receives.
    if let Some(storage) = registry.storage() {
        match storage.get(&agent_id).await {
            Ok(Some(agent)) => {
                let _ = tx.send(aap_initialize_line(&agent.config));
            }
            Ok(None) => warn!(%agent_id, "No agent record found; cannot send AAP initialize"),
            Err(e) => warn!(%agent_id, %e, "Failed to load agent for AAP initialize"),
        }
    }

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

/// Convert an AAP [`agentd_agent_protocol::Usage`] into the orchestrator's
/// [`UsageSnapshot`]. The field shapes are identical by design.
fn usage_to_snapshot(u: &agentd_agent_protocol::Usage) -> UsageSnapshot {
    UsageSnapshot {
        input_tokens: u.input_tokens,
        output_tokens: u.output_tokens,
        cache_read_input_tokens: u.cache_read_input_tokens,
        cache_creation_input_tokens: u.cache_creation_input_tokens,
        total_cost_usd: u.total_cost_usd,
        num_turns: u.num_turns,
        duration_ms: u.duration_ms,
        duration_api_ms: u.duration_api_ms,
    }
}

/// Build the AAP `initialize` NDJSON line (with trailing newline) for an agent.
///
/// This is the sole handshake the host sends before prompts: it carries all
/// agent configuration (model, system prompt, workspace, MCP servers) that the
/// adapter maps to its agent's native invocation.
pub(crate) fn aap_initialize_line(config: &crate::types::AgentConfig) -> String {
    use agentd_agent_protocol::{
        HostMessage, InitializeParams, McpServer, SystemPrompt, SystemPromptMode, Tools, Workspace,
        PROTOCOL_VERSION,
    };

    let system_prompt = match (&config.system_prompt, &config.system_prompt_file) {
        (Some(text), _) => Some(SystemPrompt {
            mode: if config.append_system_prompt {
                SystemPromptMode::Append
            } else {
                SystemPromptMode::Replace
            },
            text: Some(text.clone()),
            path: None,
        }),
        (None, Some(path)) => Some(SystemPrompt {
            mode: if config.append_system_prompt {
                SystemPromptMode::Append
            } else {
                SystemPromptMode::Replace
            },
            text: None,
            path: Some(path.clone()),
        }),
        (None, None) => None,
    };

    let tools = config.mcp_servers.as_ref().map(|servers| Tools {
        mcp_servers: servers
            .iter()
            .map(|(name, s)| {
                (
                    name.clone(),
                    McpServer {
                        command: s.command.clone(),
                        args: s.args.clone(),
                        env: s.env.clone(),
                    },
                )
            })
            .collect(),
    });

    let init = HostMessage::Initialize(InitializeParams {
        protocol_version: PROTOCOL_VERSION,
        model: config.model.clone(),
        system_prompt,
        workspace: Workspace {
            cwd: config.working_dir.clone(),
            additional_dirs: config.additional_dirs.clone(),
            worktree: config.worktree,
        },
        tools,
        resume_token: None,
    });

    // The stdin relay / ws sender writes the payload verbatim; include the
    // trailing newline so this line and the next don't concatenate.
    init.to_ndjson().unwrap_or_default() + "\n"
}

/// Generate a human-readable one-line summary of a tool call input.
fn summarize_tool_input(tool_name: &str, input: &Value) -> String {
    let truncate = |s: &str, max: usize| -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            // Find a valid UTF-8 char boundary at or before `max` to avoid
            // panicking on multi-byte characters (e.g. em-dashes, arrows).
            let end = s.floor_char_boundary(max);
            format!("{}…", &s[..end])
        }
    };

    match tool_name {
        "Bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            truncate(cmd, 100)
        }
        "Read" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            truncate(path, 100)
        }
        "Edit" | "Write" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            truncate(path, 100)
        }
        "Grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            truncate(&format!("{} in {}", pattern, path), 100)
        }
        "Glob" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            truncate(pattern, 100)
        }
        "WebFetch" => {
            let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");
            truncate(url, 100)
        }
        _ => {
            let serialized = serde_json::to_string(input).unwrap_or_default();
            truncate(&serialized, 100)
        }
    }
}

/// Record and broadcast one `agent:output` event per non-empty line in `text`.
///
/// Each line is assigned its own per-agent `seq`, persisted, and broadcast
/// with that seq in the live frame so v2 stream consumers can dedupe.
async fn broadcast_output(
    agent_id: &Uuid,
    text: &str,
    session: i64,
    registry: &ConnectionRegistry,
) {
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let seq = registry
            .record_and_seq(ConversationEvent::new(
                *agent_id,
                ConversationEventType::Output,
                session,
                Some(line.to_string()),
                None,
            ))
            .await;
        let event = serde_json::json!({
            "type": "agent:output",
            "seq": seq,
            // snake_case for the /stream/{agent_id} filter
            "agent_id": agent_id.to_string(),
            // camelCase for the frontend AgentEvent type
            "agentId": agent_id.to_string(),
            "line": line,
            "session_number": session,
            "timestamp": Utc::now().to_rfc3339(),
        });
        let _ = registry.stream_tx.send(event.to_string());
    }
}

/// Process incoming AAP NDJSON frames from an agent adapter.
///
/// Parses each line into a typed [`agentd_agent_protocol::AgentMessage`] and
/// normalizes it into the orchestrator's `ConversationEventType` vocabulary,
/// preserving the existing streaming/persistence behavior. Unparseable lines
/// are logged and skipped (per the AAP spec's forward-compat rule).
pub(crate) async fn handle_incoming_message(
    agent_id: &Uuid,
    text: &str,
    registry: &ConnectionRegistry,
) {
    use agentd_agent_protocol::{
        ActivityState as AapActivity, AgentMessage, ContentBlock, LogLevel,
    };

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: AgentMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                warn!(%agent_id, %e, "Failed to parse AAP message from agent; skipping line");
                continue;
            }
        };

        let session = registry.get_session_number(agent_id).await;

        match msg {
            AgentMessage::Ready { protocol_version, agent, capabilities, models } => {
                info!(
                    %agent_id, agent_name = %agent.name, protocol_version,
                    ?capabilities, ?models, "Agent adapter ready"
                );
                registry.set_capabilities(*agent_id, capabilities).await;
                registry.mark_ready(*agent_id).await;
            }
            AgentMessage::Message { content, .. } => {
                for block in &content {
                    match block {
                        ContentBlock::Text { text } => {
                            broadcast_output(agent_id, text, session, registry).await;
                        }
                        ContentBlock::Thinking { text } => {
                            let seq = registry
                                .record_and_seq(ConversationEvent::new(
                                    *agent_id,
                                    ConversationEventType::Thinking,
                                    session,
                                    Some(text.clone()),
                                    None,
                                ))
                                .await;
                            let stream_event = serde_json::json!({
                                "type": "agent:thinking",
                                "seq": seq,
                                "agent_id": agent_id.to_string(),
                                "agentId": agent_id.to_string(),
                                "text": text,
                                "session_number": session,
                                "timestamp": Utc::now().to_rfc3339(),
                            });
                            let _ = registry.stream_tx.send(stream_event.to_string());
                        }
                    }
                }
            }
            AgentMessage::ToolCall { call_id, name, input, .. } => {
                let summary = summarize_tool_input(&name, &input);
                let seq = registry
                    .record_and_seq(ConversationEvent::new(
                        *agent_id,
                        ConversationEventType::ToolUse,
                        session,
                        Some(summary.clone()),
                        Some(serde_json::json!({
                            "tool_name": name,
                            "tool_id": call_id,
                            "tool_input": input,
                            "summary": summary,
                        })),
                    ))
                    .await;
                let stream_event = serde_json::json!({
                    "type": "agent:tool_use",
                    "seq": seq,
                    "agent_id": agent_id.to_string(),
                    "agentId": agent_id.to_string(),
                    "tool_name": name,
                    "tool_id": call_id,
                    "tool_input": input,
                    "summary": summary,
                    "session_number": session,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                let _ = registry.stream_tx.send(stream_event.to_string());
            }
            AgentMessage::Status { state } => {
                let (activity, label) = match state {
                    AapActivity::Busy => (ActivityState::Busy, "busy"),
                    AapActivity::Idle => (ActivityState::Idle, "idle"),
                };
                registry.activity_states.write().await.insert(*agent_id, activity);
                let seq = registry
                    .record_and_seq(ConversationEvent::new(
                        *agent_id,
                        ConversationEventType::ActivityChanged,
                        session,
                        Some(label.to_string()),
                        None,
                    ))
                    .await;
                let event = serde_json::json!({
                    "type": "agent:activity_changed",
                    "seq": seq,
                    "agent_id": agent_id.to_string(),
                    "agentId": agent_id.to_string(),
                    "activity": label,
                    "session_number": session,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                let _ = registry.stream_tx.send(event.to_string());
            }
            AgentMessage::TurnComplete { is_error, result_text, usage, .. } => {
                if is_error {
                    warn!(%agent_id, "Agent turn completed with error");
                } else {
                    info!(%agent_id, "Agent turn completed successfully");
                }

                // Transition to Idle and record activity_changed with the seq
                // the snapshot will replay.
                registry.activity_states.write().await.insert(*agent_id, ActivityState::Idle);
                let activity_seq = registry
                    .record_and_seq(ConversationEvent::new(
                        *agent_id,
                        ConversationEventType::ActivityChanged,
                        session,
                        Some("idle".to_string()),
                        None,
                    ))
                    .await;
                let activity_event = serde_json::json!({
                    "type": "agent:activity_changed",
                    "seq": activity_seq,
                    "agent_id": agent_id.to_string(),
                    "agentId": agent_id.to_string(),
                    "activity": "idle",
                    "session_number": session,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                let _ = registry.stream_tx.send(activity_event.to_string());

                let usage = usage.as_ref().map(usage_to_snapshot);
                let result_text = result_text.unwrap_or_default();

                let usage_meta = usage.as_ref().map(|u| {
                    serde_json::json!({
                        "is_error": is_error,
                        "result_text": result_text,
                        "input_tokens": u.input_tokens,
                        "output_tokens": u.output_tokens,
                        "total_cost_usd": u.total_cost_usd,
                        "num_turns": u.num_turns,
                        "duration_ms": u.duration_ms,
                    })
                });
                let result_seq = registry
                    .record_and_seq(ConversationEvent::new(
                        *agent_id,
                        ConversationEventType::Result,
                        session,
                        if result_text.is_empty() { None } else { Some(result_text.clone()) },
                        usage_meta.clone(),
                    ))
                    .await;
                let result_event = serde_json::json!({
                    "type": "agent:result",
                    "seq": result_seq,
                    "agent_id": agent_id.to_string(),
                    "agentId": agent_id.to_string(),
                    "is_error": is_error,
                    "result_text": result_text,
                    "metadata": usage_meta,
                    "session_number": session,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                let _ = registry.stream_tx.send(result_event.to_string());

                if let Some(ref usage_snap) = usage {
                    let usage_payload = serde_json::json!({
                        "input_tokens": usage_snap.input_tokens,
                        "output_tokens": usage_snap.output_tokens,
                        "cache_read_input_tokens": usage_snap.cache_read_input_tokens,
                        "cache_creation_input_tokens": usage_snap.cache_creation_input_tokens,
                        "total_cost_usd": usage_snap.total_cost_usd,
                        "num_turns": usage_snap.num_turns,
                        "duration_ms": usage_snap.duration_ms,
                        "duration_api_ms": usage_snap.duration_api_ms,
                    });
                    let usage_seq = registry
                        .record_and_seq(ConversationEvent::new(
                            *agent_id,
                            ConversationEventType::UsageUpdate,
                            session,
                            None,
                            Some(usage_payload.clone()),
                        ))
                        .await;
                    let usage_event = serde_json::json!({
                        "type": "agent:usage_update",
                        "seq": usage_seq,
                        "agent_id": agent_id.to_string(),
                        "agentId": agent_id.to_string(),
                        "usage": usage_payload,
                        "session_number": session,
                        "timestamp": Utc::now().to_rfc3339(),
                    });
                    let _ = registry.stream_tx.send(usage_event.to_string());
                }

                registry
                    .notify_result(ResultInfo { agent_id: *agent_id, is_error, usage, result_text })
                    .await;
            }
            AgentMessage::ApprovalRequest { request_id, tool_name, input, .. } => {
                handle_approval_request(agent_id, request_id, tool_name, input, registry).await;
            }
            AgentMessage::Log { level, message } => match level {
                LogLevel::Error | LogLevel::Warn => warn!(%agent_id, "adapter log: {message}"),
                LogLevel::Info => debug!(%agent_id, "adapter log: {message}"),
            },
            AgentMessage::Error { fatal, code, message } => {
                warn!(%agent_id, fatal, ?code, "adapter error: {message}");
            }
        }
    }
}

/// Evaluate an AAP `approval_request` against the agent's tool policy and reply
/// with an AAP `approval_response`. The host is the sole decision authority.
async fn handle_approval_request(
    agent_id: &Uuid,
    request_id: String,
    tool_name: String,
    input: Value,
    registry: &ConnectionRegistry,
) {
    let policy = registry.get_policy(agent_id).await;
    let policy_mode = policy.mode_str();

    // sandbox_bypass takes priority — auto-approve with the sandbox disabled,
    // expressed as an opaque `updated_input` the adapter applies natively.
    if policy.matches_sandbox_bypass(&tool_name, Some(&input)) {
        info!(
            %agent_id, %tool_name, decision = "sandbox_bypass",
            "Tool call matches sandbox_bypass glob — auto-approving with sandbox disabled"
        );
        let updated = sandbox_bypass_input(&input);
        if let Err(e) = send_approval_response(
            agent_id,
            &request_id,
            ApprovalDecision::Approve,
            Some(updated),
            None,
            registry,
        )
        .await
        {
            error!(%agent_id, %e, "Failed to send sandbox-bypass approval");
        }
        return;
    }

    match policy {
        ToolPolicy::RequireApproval { .. } => {
            info!(%agent_id, %tool_name, %policy_mode, "Tool use requires human approval, holding...");
            let registry = registry.clone();
            let agent_id = *agent_id;
            tokio::spawn(async move {
                handle_approval_hold(agent_id, request_id, tool_name, input, registry).await;
            });
        }
        _ => {
            let allowed = policy.evaluate(&tool_name, Some(&input));
            if allowed {
                info!(%agent_id, %tool_name, decision = "allow", %policy_mode, "Tool use decision");
                if let Err(e) = send_approval_response(
                    agent_id,
                    &request_id,
                    ApprovalDecision::Approve,
                    Some(input.clone()),
                    None,
                    registry,
                )
                .await
                {
                    error!(%agent_id, %e, "Failed to send approval response");
                }
            } else {
                warn!(%agent_id, %tool_name, decision = "deny", %policy_mode, "Tool use decision");
                if let Err(e) = send_approval_response(
                    agent_id,
                    &request_id,
                    ApprovalDecision::Deny,
                    None,
                    Some(format!("Tool '{tool_name}': not allowed by agent policy")),
                    registry,
                )
                .await
                {
                    error!(%agent_id, %e, "Failed to send deny response");
                }
            }
        }
    }
}

/// Default approval timeout in seconds (5 minutes).
const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// Hold a tool request pending human approval.
///
/// Registers the request in the ApprovalRegistry, broadcasts a `pending_approval`
/// event on the stream, then waits for a decision (or timeout). Sends the
/// appropriate AAP `approval_response` to the adapter when resolved.
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

    // Broadcast pending_approval event for stream subscribers / UIs.
    let stream_event = serde_json::json!({
        "type": "pending_approval",
        "agent_id": agent_id,
        "approval_id": approval.id,
        "tool_name": tool_name,
        "tool_input": tool_input,
        "expires_at": approval.expires_at,
    });
    registry.broadcast(stream_event.to_string());

    let timeout = tokio::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS);
    let decision = tokio::time::timeout(timeout, rx).await;

    match decision {
        Ok(Ok(ApprovalDecision::Approve)) => {
            info!(%agent_id, %tool_name, approval_id = %approval.id, "Tool approved by human");
            if let Err(e) = send_approval_response(
                &agent_id,
                &request_id,
                ApprovalDecision::Approve,
                Some(tool_input.clone()),
                None,
                &registry,
            )
            .await
            {
                error!(%agent_id, %e, "Failed to send approve response");
            }
        }
        Ok(Ok(ApprovalDecision::Deny)) | Ok(Err(_)) => {
            warn!(%agent_id, %tool_name, approval_id = %approval.id, "Tool denied by human");
            if let Err(e) = send_approval_response(
                &agent_id,
                &request_id,
                ApprovalDecision::Deny,
                None,
                Some("denied by human operator".to_string()),
                &registry,
            )
            .await
            {
                error!(%agent_id, %e, "Failed to send deny response");
            }
        }
        Err(_elapsed) => {
            warn!(%agent_id, %tool_name, approval_id = %approval.id, "Approval timed out, auto-denying");
            registry.approvals.mark_timed_out(&approval.id).await;
            if let Err(e) = send_approval_response(
                &agent_id,
                &request_id,
                ApprovalDecision::Deny,
                None,
                Some("approval timeout — no human decision within 5 minutes".to_string()),
                &registry,
            )
            .await
            {
                error!(%agent_id, %e, "Failed to send timeout-deny response");
            }
        }
    }
}

/// Build the opaque `updated_input` for a sandbox-bypass approval by injecting
/// `dangerouslyDisableSandbox: true`. The adapter merges this into the native
/// tool call.
fn sandbox_bypass_input(input: &Value) -> Value {
    let mut updated = match input.as_object() {
        Some(map) => map.clone(),
        None => serde_json::Map::new(),
    };
    updated.insert("dangerouslyDisableSandbox".to_string(), serde_json::json!(true));
    Value::Object(updated)
}

/// Send an AAP `approval_response` to an agent adapter, mapping the internal
/// [`ApprovalDecision`] to the protocol's allow/deny.
async fn send_approval_response(
    agent_id: &Uuid,
    request_id: &str,
    decision: ApprovalDecision,
    updated_input: Option<Value>,
    message: Option<String>,
    registry: &ConnectionRegistry,
) -> anyhow::Result<()> {
    use agentd_agent_protocol::{ApprovalDecision as AapDecision, HostMessage};

    let decision = match decision {
        ApprovalDecision::Approve => AapDecision::Allow,
        ApprovalDecision::Deny => AapDecision::Deny,
    };
    let resp = HostMessage::ApprovalResponse {
        request_id: request_id.to_string(),
        decision,
        updated_input,
        message,
    };

    let connections = registry.connections.read().await;
    let conn = connections
        .get(agent_id)
        .ok_or_else(|| anyhow::anyhow!("Agent {} not connected", agent_id))?;
    conn.tx
        .send(resp.to_ndjson()? + "\n")
        .map_err(|e| anyhow::anyhow!("Failed to send to agent: {}", e))?;
    Ok(())
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

// ---------------------------------------------------------------------------
// v2 stream: snapshot + live with per-agent monotonic seq.
// ---------------------------------------------------------------------------

/// Axum handler for `GET /v2/stream/{agent_id}`.
///
/// Delivers a deterministic snapshot-then-live ordering of every conversation
/// event for the given agent. See [`handle_stream_socket_v2`] for the wire
/// protocol.
pub async fn ws_stream_agent_v2_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(registry): State<ConnectionRegistry>,
) -> impl IntoResponse {
    info!(%agent_id, "v2 stream WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_stream_socket_v2(socket, registry, agent_id))
}

/// Client → server subscribe frame for the v2 stream.
#[derive(Debug, Deserialize)]
struct V2Subscribe {
    /// Re-send only events with `seq` strictly greater than this value.
    #[serde(default)]
    since_seq: i64,
    /// Optional session filter; when set, only events from this session are
    /// included in both snapshot and live phases.
    #[serde(default)]
    session_number: Option<i64>,
}

/// Convert a persisted `ConversationEvent` into a v2 event frame.
///
/// The shape mirrors the live broadcast frame so clients render snapshot and
/// live events identically. Only one extra key — `"frame": "event"` — is
/// added on top of the live shape.
fn conversation_event_to_v2_frame(ev: &ConversationEvent) -> Value {
    let mut frame = serde_json::json!({
        "frame": "event",
        "seq": ev.seq,
        "type": format!("agent:{}", ev.event_type),
        "agent_id": ev.agent_id.to_string(),
        "agentId": ev.agent_id.to_string(),
        "session_number": ev.session_number,
        "timestamp": ev.created_at.to_rfc3339(),
    });

    let obj = frame
        .as_object_mut()
        .expect("conversation_event_to_v2_frame: json! always produces an object");

    match ev.event_type {
        ConversationEventType::Output | ConversationEventType::PromptSent => {
            if let Some(c) = ev.content.clone() {
                obj.insert("line".into(), Value::String(c));
            }
        }
        ConversationEventType::Thinking => {
            if let Some(c) = ev.content.clone() {
                obj.insert("text".into(), Value::String(c));
            }
        }
        ConversationEventType::ActivityChanged => {
            if let Some(c) = ev.content.clone() {
                obj.insert("activity".into(), Value::String(c));
            }
        }
        ConversationEventType::ToolUse => {
            // Tool metadata is stored as a flat object {tool_name, tool_id,
            // tool_input, summary} — spread it onto the frame to match the
            // live broadcast shape.
            if let Some(Value::Object(map)) = &ev.metadata {
                for (k, v) in map {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        ConversationEventType::Result => {
            if let Some(Value::Object(map)) = &ev.metadata {
                if let Some(v) = map.get("is_error") {
                    obj.insert("is_error".into(), v.clone());
                }
                if let Some(v) = map.get("result_text") {
                    obj.insert("result_text".into(), v.clone());
                }
            }
            if let Some(m) = ev.metadata.clone() {
                obj.insert("metadata".into(), m);
            }
        }
        ConversationEventType::UsageUpdate => {
            if let Some(m) = ev.metadata.clone() {
                obj.insert("usage".into(), m);
            }
        }
        ConversationEventType::ContextCleared => {}
    }

    frame
}

/// Wrap a live broadcast frame (a v1-shape JSON string) into a v2 event frame.
///
/// Returns `None` when the frame is not a conversation event for `agent_id`,
/// when it lacks a `seq` (e.g. `pending_approval` notifications), when its
/// `seq` has already been replayed in the snapshot, or when the optional
/// session filter rejects it. On success returns `(seq, frame_json_string)`.
fn live_broadcast_to_v2_frame(
    raw: &str,
    filter_agent: Uuid,
    last_replayed: i64,
    session_filter: Option<i64>,
) -> Option<(i64, String)> {
    let parsed: Value = serde_json::from_str(raw).ok()?;

    let event_agent =
        parsed.get("agent_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())?;
    if event_agent != filter_agent {
        return None;
    }

    let seq = parsed.get("seq").and_then(|v| v.as_i64())?;
    if seq <= last_replayed {
        return None;
    }

    if let Some(sf) = session_filter {
        let sn = parsed.get("session_number").and_then(|v| v.as_i64()).unwrap_or(0);
        if sn != sf {
            return None;
        }
    }

    let mut obj = parsed.as_object()?.clone();
    obj.insert("frame".to_string(), Value::String("event".to_string()));
    Some((seq, Value::Object(obj).to_string()))
}

/// v2 stream wire protocol — snapshot then live, deterministic and dedupable.
///
/// Sequence after WebSocket upgrade:
/// 1. Subscribe to the broadcast channel first so any live event broadcast
///    during the snapshot query is queued in the receiver.
/// 2. Read the client `{"frame":"subscribe","since_seq":N,"session_number":S?}`
///    frame.
/// 3. Read the snapshot cursor (`MAX(seq)` for this agent) and send
///    `{"frame":"snapshot_begin","cursor":N,"agent_id":...}`.
/// 4. Query the DB for `since_seq < seq <= cursor` and emit each row as a
///    `{"frame":"event", ...}` frame.
/// 5. Send `{"frame":"snapshot_end","seq":<last_replayed_seq>}`.
/// 6. Forward live events from the broadcast channel; drop any with
///    `seq <= last_replayed` (the dedupe boundary). On `Lagged(n)` emit a
///    `{"frame":"gap", ...}` frame and continue.
async fn handle_stream_socket_v2(socket: WebSocket, registry: ConnectionRegistry, agent_id: Uuid) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Step 1: subscribe to broadcast BEFORE the snapshot query so that any
    // event broadcast during the await window is queued in the receiver.
    let mut live_rx = registry.subscribe_stream();

    // Step 2: read the client's subscribe frame.
    let subscribe = loop {
        match ws_receiver.next().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<V2Subscribe>(&text) {
                Ok(parsed) => break parsed,
                Err(e) => {
                    let err = serde_json::json!({
                        "frame": "error",
                        "code": "bad_subscribe",
                        "message": e.to_string(),
                    });
                    let _ = ws_sender.send(Message::Text(err.to_string().into())).await;
                    return;
                }
            },
            Some(Ok(Message::Ping(_))) => continue,
            Some(Ok(Message::Close(_))) | None => return,
            _ => continue,
        }
    };

    let since_seq = subscribe.since_seq.max(0);
    let session_filter = subscribe.session_number;

    // Step 3: storage is required for the snapshot phase.
    let storage = match registry.storage() {
        Some(s) => s,
        None => {
            let err = serde_json::json!({
                "frame": "error",
                "code": "no_storage",
                "message": "orchestrator started without conversation storage",
            });
            let _ = ws_sender.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    let cursor = match storage.get_max_conversation_seq(agent_id).await {
        Ok(n) => n,
        Err(e) => {
            warn!(%agent_id, error = %e, "v2 stream: failed to read snapshot cursor");
            let err = serde_json::json!({
                "frame": "error",
                "code": "snapshot_failed",
                "message": e.to_string(),
            });
            let _ = ws_sender.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    let begin = serde_json::json!({
        "frame": "snapshot_begin",
        "cursor": cursor,
        "agent_id": agent_id.to_string(),
    });
    if ws_sender.send(Message::Text(begin.to_string().into())).await.is_err() {
        return;
    }

    // Step 4: backfill — only events strictly greater than since_seq and
    // bounded by cursor. The bound is critical: events with seq > cursor
    // are reserved for the live phase to avoid double-delivery.
    let events = match storage
        .list_conversation_events_since(agent_id, since_seq, cursor, session_filter)
        .await
    {
        Ok(list) => list,
        Err(e) => {
            warn!(%agent_id, error = %e, "v2 stream: failed to list snapshot events");
            let err = serde_json::json!({
                "frame": "error",
                "code": "snapshot_failed",
                "message": e.to_string(),
            });
            let _ = ws_sender.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    let mut last_replayed = since_seq;
    for ev in events {
        let seq = ev.seq;
        let frame = conversation_event_to_v2_frame(&ev);
        if ws_sender.send(Message::Text(frame.to_string().into())).await.is_err() {
            return;
        }
        last_replayed = last_replayed.max(seq);
    }
    // No events past since_seq in the snapshot — surface the cursor anyway so
    // the client knows "you're caught up through cursor" and can dedupe live
    // events arriving with seq <= cursor that were queued during the await.
    last_replayed = last_replayed.max(cursor);

    let end = serde_json::json!({
        "frame": "snapshot_end",
        "seq": last_replayed,
    });
    if ws_sender.send(Message::Text(end.to_string().into())).await.is_err() {
        return;
    }

    info!(%agent_id, %since_seq, %cursor, "v2 stream snapshot complete");

    // Step 5: forward live frames. The receiver already buffered everything
    // that arrived between subscribe and now; recv() will drain those first,
    // then block on new events.
    //
    // Two concurrent tasks: a forwarder (broadcast → ws) and a drain (ws →
    // close detection). They share a cancellation token so either side
    // can tear down the connection cleanly.
    let cancel = Arc::new(tokio::sync::Notify::new());

    let cancel_send = cancel.clone();
    let send_task = tokio::spawn(async move {
        let mut last_seen = last_replayed;
        loop {
            tokio::select! {
                _ = cancel_send.notified() => break,
                msg = live_rx.recv() => match msg {
                    Ok(raw) => {
                        if let Some((seq, frame)) =
                            live_broadcast_to_v2_frame(&raw, agent_id, last_seen, session_filter)
                        {
                            if ws_sender.send(Message::Text(frame.into())).await.is_err() {
                                break;
                            }
                            last_seen = last_seen.max(seq);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(%agent_id, skipped = n, "v2 stream lagged");
                        let gap = serde_json::json!({
                            "frame": "gap",
                            "skipped": n,
                            "reason": "broadcast_lagged",
                            "since_seq": last_seen,
                        });
                        if ws_sender.send(Message::Text(gap.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }

    cancel.notify_waiters();
    send_task.abort();
    info!(%agent_id, "v2 stream WebSocket connection ended");
}

// ---------------------------------------------------------------------------
// Terminal relay WebSocket  (GET /terminal/{agent_id})
// ---------------------------------------------------------------------------

/// Control message sent from a terminal client to the server.
///
/// Binary frames are forwarded as-is to the PTY stdin.
/// Text frames are parsed as JSON control messages (e.g., resize).
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalControlMessage {
    /// Resize the PTY to the given dimensions.
    Resize { cols: u16, rows: u16 },
}

/// State passed to the terminal WebSocket handler.
///
/// Carries the `AgentManager` so the handler can resolve agent → session
/// and obtain the `PtyOutputStream`.
#[derive(Clone)]
pub struct TerminalRelayState {
    pub manager: Arc<AgentManager>,
}

/// Maximum byte length accepted for a single binary stdin frame forwarded to
/// the PTY. Frames larger than this are dropped with a warning. This prevents
/// a misbehaving client from holding the PTY writer mutex for an arbitrarily
/// long write.
const MAX_STDIN_FRAME_BYTES: usize = 64 * 1024; // 64 KiB

/// Axum handler for the PTY terminal relay at `GET /terminal/{agent_id}`.
///
/// Upgrades the connection to a binary WebSocket relay:
/// - **Server → Client**: raw PTY output bytes as binary frames
/// - **Client → Server**: binary frames forwarded to PTY stdin;
///   text frames parsed as JSON control messages (e.g. `{"type":"resize","cols":120,"rows":40}`)
///
/// Returns `404 Not Found` when the agent does not exist or the backend does
/// not support PTY streaming.
/// Returns `500 Internal Server Error` when the PTY stream lookup fails.
pub async fn ws_terminal_handler(
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(state): State<TerminalRelayState>,
) -> impl IntoResponse {
    info!(%agent_id, "Terminal WebSocket upgrade request");
    // Resolve the PTY stream before upgrading so HTTP-level clients receive a
    // proper status code (404/500) rather than a WebSocket-framed error message.
    match state.manager.get_agent_pty_stream(&agent_id).await {
        Ok(None) => {
            warn!(%agent_id, "Terminal relay: no PTY stream available");
            axum::http::StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => {
            error!(%agent_id, %e, "Terminal relay: failed to get PTY stream");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
        Ok(Some(stream)) => ws
            .on_upgrade(move |socket| {
                handle_terminal_socket(socket, agent_id, stream, state.manager)
            })
            .into_response(),
    }
}

async fn handle_terminal_socket(
    socket: WebSocket,
    agent_id: Uuid,
    stream: wrap::pty_stream::PtyOutputStream,
    manager: Arc<AgentManager>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Subscribe before sending history to avoid a race between replay and live.
    let (history, mut pty_rx) = stream.subscribe();
    info!(%agent_id, history_chunks = history.len(), "Terminal relay connected");

    // Send history buffer as individual binary frames.
    for chunk in history {
        if ws_sender.send(Message::Binary(chunk)).await.is_err() {
            return;
        }
    }

    // Spawn a task that forwards live PTY output → WebSocket binary frames.
    let send_task = tokio::spawn(async move {
        loop {
            match pty_rx.recv().await {
                Ok(chunk) => {
                    if ws_sender.send(Message::Binary(chunk)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(%agent_id, skipped = n, "Terminal relay lagged, skipped PTY chunks");
                    // Continue — we already lost these bytes; keep forwarding.
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!(%agent_id, "PTY broadcast channel closed, terminal relay ending");
                    break;
                }
            }
        }
    });

    // Main loop: handle incoming client frames.
    loop {
        match ws_receiver.next().await {
            None => break, // client closed the connection cleanly
            Some(Err(e)) => {
                warn!(%agent_id, %e, "Terminal relay: receive error");
                break;
            }
            Some(Ok(msg)) => match msg {
                Message::Binary(data) => {
                    // Raw keyboard input — forward to PTY stdin.
                    if data.len() > MAX_STDIN_FRAME_BYTES {
                        warn!(
                            %agent_id,
                            bytes = data.len(),
                            limit = MAX_STDIN_FRAME_BYTES,
                            "Terminal relay: oversized stdin frame dropped"
                        );
                    } else if let Err(e) = stream.write_input(&data) {
                        warn!(%agent_id, %e, "Terminal relay: failed to write input to PTY");
                    }
                }
                Message::Text(text) => {
                    // JSON control message — parse and dispatch.
                    match serde_json::from_str::<TerminalControlMessage>(&text) {
                        Ok(TerminalControlMessage::Resize { cols, rows }) => {
                            debug!(%agent_id, cols, rows, "Terminal resize request");
                            if let Err(e) = manager.resize_agent_pty(&agent_id, cols, rows).await {
                                warn!(%agent_id, %e, "Terminal relay: resize failed");
                            }
                        }
                        Err(e) => {
                            warn!(%agent_id, %e, text = %text, "Terminal relay: unrecognised control message");
                        }
                    }
                }
                Message::Close(_) => {
                    info!(%agent_id, "Terminal client disconnected");
                    break;
                }
                Message::Ping(_) => {} // auto-pong by axum
                _ => {}
            },
        }
    }

    send_task.abort();
    // Wait for the send task to exit so its ws_sender is dropped before we return.
    let _ = send_task.await;
    info!(%agent_id, "Terminal WebSocket connection ended");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_to_snapshot_maps_all_fields() {
        let u = agentd_agent_protocol::Usage {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_input_tokens: 3,
            cache_creation_input_tokens: 4,
            total_cost_usd: 0.5,
            num_turns: 6,
            duration_ms: 7,
            duration_api_ms: 8,
        };
        let s = usage_to_snapshot(&u);
        assert_eq!(s.input_tokens, 1);
        assert_eq!(s.output_tokens, 2);
        assert_eq!(s.cache_read_input_tokens, 3);
        assert_eq!(s.cache_creation_input_tokens, 4);
        assert!((s.total_cost_usd - 0.5).abs() < 1e-9);
        assert_eq!(s.num_turns, 6);
        assert_eq!(s.duration_ms, 7);
        assert_eq!(s.duration_api_ms, 8);
    }

    #[test]
    fn aap_initialize_line_maps_config() {
        use crate::types::{AgentConfig, ToolPolicy};
        let config = AgentConfig {
            working_dir: "/repo".to_string(),
            user: None,
            shell: "zsh".to_string(),
            interactive: false,
            prompt: None,
            worktree: true,
            system_prompt: Some("hi".to_string()),
            system_prompt_file: None,
            append_system_prompt: true,
            tool_policy: ToolPolicy::default(),
            model: Some("opus".to_string()),
            env: std::collections::HashMap::new(),
            auto_clear_threshold: None,
            network_policy: None,
            docker_image: None,
            extra_mounts: None,
            resource_limits: None,
            additional_dirs: vec!["/x".to_string()],
            rooms: vec![],
            mcp_servers: None,
            agent_type: "claude".to_string(),
        };
        let line = aap_initialize_line(&config);
        let v: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["type"], "initialize");
        assert_eq!(v["protocol_version"], 1);
        assert_eq!(v["model"], "opus");
        assert_eq!(v["workspace"]["cwd"], "/repo");
        assert_eq!(v["workspace"]["worktree"], true);
        assert_eq!(v["workspace"]["additional_dirs"][0], "/x");
        assert_eq!(v["system_prompt"]["mode"], "append");
        assert_eq!(v["system_prompt"]["text"], "hi");
    }

    #[tokio::test]
    async fn aap_ready_marks_agent_ready_and_records_capabilities() {
        let registry = ConnectionRegistry::new();
        let id = Uuid::new_v4();
        let line = r#"{"type":"ready","protocol_version":1,"agent":{"name":"claude-code"},"capabilities":["tool_approval","streaming"]}"#;
        handle_incoming_message(&id, line, &registry).await;
        assert!(registry.is_ready(&id).await);
        let caps = registry.get_capabilities(&id).await.expect("capabilities recorded");
        assert!(caps.iter().any(|c| c == "tool_approval"));
    }

    #[tokio::test]
    async fn aap_turn_complete_notifies_result_with_usage() {
        let registry = ConnectionRegistry::new();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = captured.clone();
        registry
            .on_result(std::sync::Arc::new(move |info| {
                *sink.lock().unwrap() = Some(info);
            }))
            .await;
        let id = Uuid::new_v4();
        let line = r#"{"type":"turn_complete","turn_id":"t1","is_error":false,"result_text":"done","usage":{"input_tokens":10,"output_tokens":5,"total_cost_usd":0.01,"num_turns":1}}"#;
        handle_incoming_message(&id, line, &registry).await;
        let info = captured.lock().unwrap().clone().expect("result callback fired");
        assert!(!info.is_error);
        assert_eq!(info.result_text, "done");
        let usage = info.usage.expect("usage present");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
    }

    #[tokio::test]
    async fn aap_unparseable_line_is_skipped() {
        let registry = ConnectionRegistry::new();
        let id = Uuid::new_v4();
        // Must not panic on garbage or unknown message types.
        handle_incoming_message(&id, "not json", &registry).await;
        handle_incoming_message(&id, r#"{"type":"totally_unknown"}"#, &registry).await;
        assert!(!registry.is_ready(&id).await);
    }

    // ---------------------------------------------------------------------------
    // Activity state tests
    // ---------------------------------------------------------------------------

    #[tokio::test]
    async fn test_new_connection_defaults_to_idle() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();

        // Before connection: unknown agent should default to Idle.
        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Idle);
    }

    #[tokio::test]
    async fn test_register_sets_idle() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Idle);
    }

    #[tokio::test]
    async fn test_unregister_removes_activity_state() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        // Manually set Busy to verify unregister clears it.
        registry.activity_states.write().await.insert(agent_id, ActivityState::Busy);
        registry.unregister(&agent_id).await;

        // After unregister: defaults back to Idle (no entry in map).
        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Idle);
    }

    #[tokio::test]
    async fn test_send_user_message_transitions_to_busy() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Idle);

        registry.send_user_message(&agent_id, "hello").await.unwrap();

        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Busy);
    }

    #[tokio::test]
    async fn test_result_message_transitions_to_idle() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        // Simulate busy state.
        registry.activity_states.write().await.insert(agent_id, ActivityState::Busy);
        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Busy);

        // Simulate receiving an AAP turn_complete message.
        let result_msg = json!({
            "type": "turn_complete",
            "turn_id": "t1",
            "is_error": false,
            "result_text": "done",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
            }
        });
        handle_incoming_message(&agent_id, &result_msg.to_string(), &registry).await;

        assert_eq!(registry.get_activity_state(&agent_id).await, ActivityState::Idle);
    }

    #[tokio::test]
    async fn test_result_message_broadcasts_activity_changed_event() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let mut stream_rx = registry.subscribe_stream();

        // Set busy then receive result.
        registry.activity_states.write().await.insert(agent_id, ActivityState::Busy);
        let result_msg = json!({
            "type": "turn_complete",
            "turn_id": "t1",
            "is_error": false,
            "result_text": "",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        });
        handle_incoming_message(&agent_id, &result_msg.to_string(), &registry).await;

        // Drain broadcast messages looking for the activity_changed event.
        let mut found = false;
        while let Ok(msg) = stream_rx.try_recv() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
                if v.get("type").and_then(|t| t.as_str()) == Some("agent:activity_changed")
                    && v.get("activity").and_then(|a| a.as_str()) == Some("idle")
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Expected agent:activity_changed (idle) event on stream");
    }

    #[tokio::test]
    async fn test_send_user_message_broadcasts_activity_changed_event() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let mut stream_rx = registry.subscribe_stream();

        registry.send_user_message(&agent_id, "test prompt").await.unwrap();

        // Drain looking for the busy activity_changed event.
        let mut found = false;
        while let Ok(msg) = stream_rx.try_recv() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&msg) {
                if v.get("type").and_then(|t| t.as_str()) == Some("agent:activity_changed")
                    && v.get("activity").and_then(|a| a.as_str()) == Some("busy")
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Expected agent:activity_changed (busy) event on stream");
    }

    // -----------------------------------------------------------------------
    // Terminal control message parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_terminal_resize_message_deserialises() {
        let json = r#"{"type":"resize","cols":120,"rows":40}"#;
        let msg: TerminalControlMessage = serde_json::from_str(json).unwrap();
        match msg {
            TerminalControlMessage::Resize { cols, rows } => {
                assert_eq!(cols, 120);
                assert_eq!(rows, 40);
            }
        }
    }

    #[test]
    fn test_terminal_resize_message_serialises() {
        let msg = TerminalControlMessage::Resize { cols: 80, rows: 24 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"resize""#));
        assert!(json.contains(r#""cols":80"#));
        assert!(json.contains(r#""rows":24"#));
    }

    #[test]
    fn test_terminal_unknown_message_type_errors() {
        let json = r#"{"type":"unknown_action","data":"foo"}"#;
        let result = serde_json::from_str::<TerminalControlMessage>(json);
        assert!(result.is_err(), "Unknown type should fail to deserialise");
    }

    #[test]
    fn test_terminal_resize_missing_fields_errors() {
        // Missing `rows` — should fail.
        let json = r#"{"type":"resize","cols":80}"#;
        let result = serde_json::from_str::<TerminalControlMessage>(json);
        assert!(result.is_err(), "Resize missing rows should fail");
    }

    #[test]
    fn test_terminal_relay_state_is_clone() {
        // Verifies the TerminalRelayState can be cloned (required by axum State).
        fn assert_clone<T: Clone>() {}
        assert_clone::<TerminalRelayState>();
    }

    // -----------------------------------------------------------------------
    // Persistence tests
    // -----------------------------------------------------------------------

    /// Build a temporary AgentStorage backed by a SQLite file in a TempDir.
    async fn create_test_storage() -> (crate::storage::AgentStorage, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let db =
            crate::storage::AgentStorage::with_path(&tmp.path().join("test.db")).await.unwrap();
        (db, tmp)
    }

    #[tokio::test]
    async fn test_register_no_storage_session_counter_defaults_to_zero() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        assert_eq!(registry.get_session_number(&agent_id).await, 0);
    }

    #[tokio::test]
    async fn test_unregister_removes_session_counter_entry() {
        let registry = ConnectionRegistry::new();
        let agent_id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        // Counter exists after register.
        assert!(registry.session_numbers.read().await.contains_key(&agent_id));

        registry.unregister(&agent_id).await;

        // Counter removed - get_session_number falls back to the default 0.
        assert!(!registry.session_numbers.read().await.contains_key(&agent_id));
        assert_eq!(registry.get_session_number(&agent_id).await, 0);
    }

    #[tokio::test]
    async fn test_register_with_storage_reads_max_session_from_events() {
        use crate::types::{ConversationEvent, ConversationEventType};

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        // Pre-seed two events at session_number 2 (simulating a reconnecting agent).
        for _ in 0..2 {
            let ev = ConversationEvent::new(
                agent_id,
                ConversationEventType::Output,
                2,
                Some("line".to_string()),
                None,
            );
            storage.insert_conversation_event(&ev).await.unwrap();
        }

        let registry = ConnectionRegistry::new().with_storage(storage);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        // Give the fire-and-forget task time to complete.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(
            registry.get_session_number(&agent_id).await,
            2,
            "session counter should be restored to MAX(session_number) = 2"
        );
    }

    #[tokio::test]
    async fn test_persist_context_cleared_updates_counter_and_writes_event() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        registry.persist_context_cleared(agent_id, 1).await;

        // Give the fire-and-forget task time to complete.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Counter updated synchronously by persist_context_cleared.
        assert_eq!(registry.get_session_number(&agent_id).await, 1);

        // Event persisted asynchronously.
        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, ConversationEventType::ContextCleared);
    }

    #[tokio::test]
    async fn test_send_user_message_persists_prompt_sent_and_activity_changed() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        registry.send_user_message(&agent_id, "hello world").await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();

        assert_eq!(events.len(), 2, "expected prompt_sent + activity_changed(busy)");

        let types: Vec<_> = events.iter().map(|e| &e.event_type).collect();
        assert!(types.contains(&&ConversationEventType::PromptSent), "expected PromptSent event");
        assert!(
            types.contains(&&ConversationEventType::ActivityChanged),
            "expected ActivityChanged event"
        );

        let prompt_sent =
            events.iter().find(|e| e.event_type == ConversationEventType::PromptSent).unwrap();
        assert_eq!(prompt_sent.content.as_deref(), Some("hello world"));
    }

    #[tokio::test]
    async fn test_handle_incoming_message_assistant_output_persisted() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let msg = serde_json::json!({
            "type": "message",
            "turn_id": "t1",
            "content": [{"type": "text", "text": "Here is my answer."}]
        });
        handle_incoming_message(&agent_id, &msg.to_string(), &registry).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();

        assert!(
            events.iter().any(|e| e.event_type == ConversationEventType::Output),
            "expected Output event from assistant message"
        );
    }

    #[tokio::test]
    async fn test_handle_incoming_message_tool_use_persisted() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let msg = serde_json::json!({
            "type": "tool_call",
            "turn_id": "t1",
            "call_id": "tool_abc123",
            "name": "Bash",
            "input": {"command": "ls"}
        });
        handle_incoming_message(&agent_id, &msg.to_string(), &registry).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();

        assert!(
            events.iter().any(|e| e.event_type == ConversationEventType::ToolUse),
            "expected ToolUse event from assistant message"
        );
    }

    #[tokio::test]
    async fn test_handle_incoming_message_thinking_persisted() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let msg = serde_json::json!({
            "type": "message",
            "turn_id": "t1",
            "content": [{
                "type": "thinking",
                "text": "Let me reason through this."
            }]
        });
        handle_incoming_message(&agent_id, &msg.to_string(), &registry).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();

        assert!(
            events.iter().any(|e| e.event_type == ConversationEventType::Thinking),
            "expected Thinking event from assistant message"
        );
    }

    #[tokio::test]
    async fn test_handle_incoming_message_result_persists_result_and_idle() {
        use crate::types::ConversationEventType;

        let (storage, _tmp) = create_test_storage().await;
        let agent_id = Uuid::new_v4();

        let registry = ConnectionRegistry::new().with_storage(storage.clone());
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        registry.register(agent_id, AgentConnection { tx }).await;

        let msg = serde_json::json!({
            "type": "turn_complete",
            "turn_id": "t1",
            "is_error": false,
            "result_text": "done",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            }
        });
        handle_incoming_message(&agent_id, &msg.to_string(), &registry).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events = storage
            .list_conversation_events(agent_id, &crate::types::ConversationQuery::default())
            .await
            .unwrap();

        let types: Vec<_> = events.iter().map(|e| &e.event_type).collect();
        assert!(types.contains(&&ConversationEventType::Result), "expected Result event");
        assert!(
            types.contains(&&ConversationEventType::ActivityChanged),
            "expected ActivityChanged(idle) event"
        );
    }
}
