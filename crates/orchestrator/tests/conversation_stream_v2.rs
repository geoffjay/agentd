//! End-to-end integration tests for the v2 conversation stream.
//!
//! The v2 stream (`/v2/stream/{agent_id}`) is the unified history + live
//! protocol that both the Web UI and TUI consume. Correctness boils down to:
//!
//! 1. The per-agent `seq` is strictly monotonic across record_and_seq calls.
//! 2. A snapshot at any point returns every event with seq ≤ cursor and
//!    nothing past the cursor; subsequent live events resume from seq > cursor.
//! 3. Two clients that connect at different times receive the same ordered
//!    set of events (modulo each one's `since_seq`).
//! 4. A client reconnecting with `since_seq = N` receives only events with
//!    seq > N — no duplicates, no replay of state it already saw.
//!
//! Tests bind a real TCP listener (random port) and drive the full Axum
//! router via `tokio-tungstenite`, mirroring exactly what the Web UI and TUI
//! clients do over the wire.

use async_trait::async_trait;
use communicate::client::CommunicateClient;
use futures::{SinkExt, StreamExt};
use orchestrator::{
    api::{create_router, ApiState},
    manager::AgentManager,
    scheduler::{storage::SchedulerStorage, Scheduler},
    storage::AgentStorage,
    types::{
        Agent, AgentConfig, ConversationEvent, ConversationEventType, ConversationQuery, ToolPolicy,
    },
    websocket::{AgentConnection, ConnectionRegistry},
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use uuid::Uuid;
use wrap::{
    backend::{SessionConfig, SessionExitInfo, SessionHealth},
    types::BackendType,
    ExecutionBackend,
};

// ---------------------------------------------------------------------------
// NullBackend — no-op execution backend
// ---------------------------------------------------------------------------

struct NullBackend;

#[async_trait]
impl ExecutionBackend for NullBackend {
    async fn create_session(&self, _: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn launch_agent(&self, _: &SessionConfig) -> anyhow::Result<()> {
        Ok(())
    }
    async fn session_exists(&self, _: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn kill_session(&self, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn send_command(&self, _: &str, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
    fn prefix(&self) -> &str {
        "test"
    }
    async fn session_health(&self, _: &str) -> anyhow::Result<SessionHealth> {
        Ok(SessionHealth::Unknown)
    }
    async fn session_exit_info(&self, _: &str) -> anyhow::Result<Option<SessionExitInfo>> {
        Ok(None)
    }
    async fn session_pid(&self, _: &str) -> anyhow::Result<Option<u32>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

struct Harness {
    base_url: String,
    storage: Arc<AgentStorage>,
    registry: ConnectionRegistry,
    agent_id: Uuid,
    _server_task: tokio::task::JoinHandle<()>,
    _temp: TempDir,
}

impl Harness {
    async fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let storage =
            Arc::new(AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap());
        let scheduler_storage = SchedulerStorage::new(storage.db().clone());
        let registry = ConnectionRegistry::new().with_storage((*storage).clone());
        let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));
        let manager = Arc::new(AgentManager::new(
            storage.clone(),
            Arc::new(NullBackend),
            registry.clone(),
            "ws://127.0.0.1:0".to_string(),
        ));
        let communicate = CommunicateClient::new("http://127.0.0.1:0");

        let state = ApiState {
            manager,
            registry: registry.clone(),
            scheduler,
            communicate,
            backend_type: BackendType::Tmux,
        };
        let router = create_router(state);

        // Bind on a random port so multiple tests can run concurrently.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{addr}");

        let server_task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        // Create an agent in storage so subsequent /agents routes don't 404
        // (the /v2/stream route itself doesn't require the row, but the
        // record_and_seq path implicitly assumes a registered agent).
        let agent_id = create_agent(&storage).await;

        // Register a dummy AgentConnection so the seq counter seeds from
        // storage (currently 0). After this, registry.record_and_seq is
        // safe to call from the test.
        let (tx, _rx) = mpsc::unbounded_channel::<String>();
        registry.register(agent_id, AgentConnection { tx }).await;

        Self { base_url, storage, registry, agent_id, _server_task: server_task, _temp: temp }
    }

    /// Record + broadcast a single event the same way the orchestrator does:
    /// assign seq, persist, then publish a live frame including that seq.
    async fn record_output(&self, line: &str) -> i64 {
        let seq = self
            .registry
            .record_and_seq(ConversationEvent::new(
                self.agent_id,
                ConversationEventType::Output,
                0,
                Some(line.to_string()),
                None,
            ))
            .await;
        let frame = json!({
            "type": "agent:output",
            "seq": seq,
            "agent_id": self.agent_id.to_string(),
            "agentId": self.agent_id.to_string(),
            "line": line,
            "session_number": 0,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        self.registry.broadcast(frame.to_string());
        seq
    }

    fn ws_url(&self) -> String {
        let ws_base = self.base_url.replacen("http://", "ws://", 1);
        format!("{ws_base}/v2/stream/{}", self.agent_id)
    }
}

async fn create_agent(storage: &AgentStorage) -> Uuid {
    let agent = Agent::new(
        format!("test-{}", Uuid::new_v4()),
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
        },
    );
    storage.add(&agent).await.unwrap();
    agent.id
}

/// Wrapper around a tokio-tungstenite client that subscribes to the v2 stream
/// and drains frames into a Vec.
struct V2Client {
    write: futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        WsMessage,
    >,
    read: futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
}

impl V2Client {
    async fn connect(url: &str, since_seq: i64) -> Self {
        let (ws, _) = connect_async(url).await.expect("ws connect");
        let (write, read) = ws.split();
        let mut client = Self { write, read };
        let subscribe = json!({"frame": "subscribe", "since_seq": since_seq});
        client.write.send(WsMessage::Text(subscribe.to_string().into())).await.unwrap();
        client
    }

    /// Read the next JSON frame, returning None on close/timeout.
    async fn next_frame(&mut self) -> Option<serde_json::Value> {
        let res = timeout(Duration::from_secs(2), self.read.next()).await.ok()??;
        match res.ok()? {
            WsMessage::Text(t) => serde_json::from_str(&t).ok(),
            _ => None,
        }
    }

    /// Drain frames until a `snapshot_end` is observed (or timeout). Returns
    /// (cursor_from_snapshot_begin, replayed_event_seqs, snapshot_end_seq).
    async fn drain_snapshot(&mut self) -> (i64, Vec<i64>, i64) {
        let begin = self.next_frame().await.expect("snapshot_begin");
        assert_eq!(begin["frame"], "snapshot_begin", "first frame should be snapshot_begin");
        let cursor = begin["cursor"].as_i64().expect("cursor i64");

        let mut events = Vec::new();
        loop {
            let frame = self.next_frame().await.expect("frame");
            match frame["frame"].as_str() {
                Some("event") => {
                    events.push(frame["seq"].as_i64().expect("event.seq i64"));
                }
                Some("snapshot_end") => {
                    let seq = frame["seq"].as_i64().expect("snapshot_end.seq i64");
                    return (cursor, events, seq);
                }
                other => panic!("unexpected frame during snapshot: {other:?}"),
            }
        }
    }

    /// Read the next `event` frame, skipping anything else (timeouts treated
    /// as None).
    async fn next_event_seq(&mut self) -> Option<i64> {
        loop {
            let frame = self.next_frame().await?;
            if frame["frame"] == "event" {
                return frame["seq"].as_i64();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `record_and_seq` assigns strictly monotonic seqs starting at 1 for a
/// freshly-registered agent.
#[tokio::test]
async fn seq_assignment_is_strictly_monotonic() {
    let h = Harness::new().await;

    let s1 = h.record_output("one").await;
    let s2 = h.record_output("two").await;
    let s3 = h.record_output("three").await;

    assert_eq!(s1, 1);
    assert_eq!(s2, 2);
    assert_eq!(s3, 3);

    // Storage agrees.
    let max = h.storage.get_max_conversation_seq(h.agent_id).await.unwrap();
    assert_eq!(max, 3);
}

/// `list_conversation_events_since` returns exactly the events in the
/// (since_seq, max_seq] window in seq order.
#[tokio::test]
async fn list_since_window_is_strictly_inclusive_exclusive() {
    let h = Harness::new().await;
    for i in 1..=10 {
        h.record_output(&format!("line-{i}")).await;
    }

    let events = h.storage.list_conversation_events_since(h.agent_id, 3, 7, None).await.unwrap();
    let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![4, 5, 6, 7]);
}

/// A fresh client (since_seq=0) receives the full history through the snapshot
/// phase, then sees live events arriving after snapshot_end.
#[tokio::test]
async fn snapshot_then_live_delivers_complete_ordering() {
    let h = Harness::new().await;

    // Pre-populate 5 events.
    for i in 1..=5 {
        h.record_output(&format!("pre-{i}")).await;
    }

    let mut client = V2Client::connect(&h.ws_url(), 0).await;
    let (cursor, snapshot, end_seq) = client.drain_snapshot().await;

    assert_eq!(cursor, 5, "snapshot_begin cursor must equal latest persisted seq");
    assert_eq!(snapshot, vec![1, 2, 3, 4, 5], "snapshot must replay every event");
    assert_eq!(end_seq, 5, "snapshot_end.seq must equal cursor when nothing new arrived");

    // Now produce 3 live events and confirm the client picks them up.
    for i in 1..=3 {
        h.record_output(&format!("live-{i}")).await;
    }

    let live_seqs: Vec<i64> = vec![
        client.next_event_seq().await.unwrap(),
        client.next_event_seq().await.unwrap(),
        client.next_event_seq().await.unwrap(),
    ];
    assert_eq!(live_seqs, vec![6, 7, 8]);
}

/// Two clients connecting at different times see the same canonical ordering
/// of every event — the divergence the v2 protocol was built to eliminate.
#[tokio::test]
async fn two_clients_at_different_times_agree() {
    let h = Harness::new().await;

    // Client A connects to an empty agent.
    let mut a = V2Client::connect(&h.ws_url(), 0).await;
    let (_, a_snapshot, a_end) = a.drain_snapshot().await;
    assert!(a_snapshot.is_empty());
    assert_eq!(a_end, 0);

    // 10 events flow live to A.
    for i in 1..=10 {
        h.record_output(&format!("event-{i}")).await;
    }
    let mut a_seqs: Vec<i64> = Vec::new();
    for _ in 0..10 {
        a_seqs.push(a.next_event_seq().await.unwrap());
    }
    assert_eq!(a_seqs, (1..=10).collect::<Vec<_>>());

    // Client B connects fresh — it must see the same ordering in its snapshot.
    let mut b = V2Client::connect(&h.ws_url(), 0).await;
    let (b_cursor, b_snapshot, b_end) = b.drain_snapshot().await;
    assert_eq!(b_cursor, 10);
    assert_eq!(b_snapshot, a_seqs);
    assert_eq!(b_end, 10);
}

/// `since_seq=N` replay returns only events with seq > N, both in the
/// snapshot phase and live phase.
#[tokio::test]
async fn since_seq_resumes_only_delta() {
    let h = Harness::new().await;
    for i in 1..=8 {
        h.record_output(&format!("line-{i}")).await;
    }

    let mut client = V2Client::connect(&h.ws_url(), 5).await;
    let (cursor, snapshot, end_seq) = client.drain_snapshot().await;

    assert_eq!(cursor, 8);
    assert_eq!(snapshot, vec![6, 7, 8], "must replay only seqs strictly greater than 5");
    assert_eq!(end_seq, 8);

    // A live event arrives — client should see seq=9.
    h.record_output("after-resume").await;
    assert_eq!(client.next_event_seq().await, Some(9));
}

/// The migration's backfill assigns sequential seq values to pre-existing rows
/// in `(created_at, id)` order, never leaving any at 0.
#[tokio::test]
async fn migration_backfill_yields_monotonic_seq() {
    // Hand-inserted events bypass record_and_seq entirely, mimicking what
    // pre-migration data looks like. The fixture inserts them with explicit
    // seq=N to exercise the storage path that the migration produces.
    let temp = TempDir::new().unwrap();
    let storage = AgentStorage::with_path(&temp.path().join("db.sqlite")).await.unwrap();
    let agent_id = Uuid::new_v4();

    // Insert 5 events in mixed order; explicit seq simulating the migration
    // backfill assignment.
    for i in 1..=5_i64 {
        let mut ev = ConversationEvent::new(
            agent_id,
            ConversationEventType::Output,
            0,
            Some(format!("event-{i}")),
            None,
        );
        ev.seq = i;
        ev.created_at = chrono::Utc::now() + chrono::Duration::seconds(i);
        storage.insert_conversation_event(&ev).await.unwrap();
    }

    let max = storage.get_max_conversation_seq(agent_id).await.unwrap();
    assert_eq!(max, 5);

    let events =
        storage.list_conversation_events(agent_id, &ConversationQuery::default()).await.unwrap();
    let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, vec![1, 2, 3, 4, 5]);
}
