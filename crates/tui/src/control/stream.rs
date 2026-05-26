use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// Spawn a background task that connects to the orchestrator's v2 conversation
/// stream and feeds parsed frames into an `UnboundedReceiver`.
///
/// The task automatically reconnects on disconnect or error: it tracks the
/// highest `seq` it has observed and resubscribes with `since_seq = last_seq`
/// so the server's snapshot replays only the delta. This makes the TUI
/// resilient to network blips, server restarts, and broadcast lag — without
/// the reconnect loop the TUI would silently truncate at the disconnect
/// point while the Web UI (which uses an auto-reconnecting WebSocket
/// manager) keeps receiving.
///
/// The returned receiver is closed only when the caller drops it. Aborting
/// the returned `AbortHandle` cancels the task immediately.
///
/// Frame shapes — see `crates/orchestrator/src/websocket.rs` for the wire
/// protocol — are passed through as raw `serde_json::Value`s.
pub fn spawn(
    base_url: &str,
    agent_id: Uuid,
    initial_since_seq: i64,
) -> (mpsc::UnboundedReceiver<serde_json::Value>, tokio::task::AbortHandle) {
    let (tx, rx) = mpsc::unbounded_channel();

    let ws_url = base_url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let url = format!("{ws_url}/v2/stream/{agent_id}");

    let handle = tokio::spawn(async move {
        let mut last_seq = initial_since_seq;
        let mut backoff = Duration::from_millis(200);
        let max_backoff = Duration::from_secs(5);

        loop {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    backoff = Duration::from_millis(200);
                    let (mut write, mut read) = ws_stream.split();

                    let subscribe = serde_json::json!({
                        "frame": "subscribe",
                        "since_seq": last_seq,
                    });
                    if write.send(Message::Text(subscribe.to_string().into())).await.is_err() {
                        tracing::warn!(
                            "v2 stream: subscribe send failed for agent {agent_id} — reconnecting"
                        );
                    } else {
                        // Drain frames until the connection ends.
                        loop {
                            match read.next().await {
                                Some(Ok(Message::Text(text))) => {
                                    let Ok(value) =
                                        serde_json::from_str::<serde_json::Value>(&text)
                                    else {
                                        continue;
                                    };
                                    // Track the highest seq we have seen so a
                                    // reconnect resumes from the right cursor.
                                    let frame = value.get("frame").and_then(|v| v.as_str());
                                    if matches!(frame, Some("event") | Some("snapshot_end")) {
                                        if let Some(seq) = value.get("seq").and_then(|v| v.as_i64())
                                        {
                                            if seq > last_seq {
                                                last_seq = seq;
                                            }
                                        }
                                    }
                                    if tx.send(value).is_err() {
                                        // Receiver dropped — caller no longer cares.
                                        return;
                                    }
                                }
                                Some(Ok(Message::Close(_))) | None => {
                                    tracing::info!(
                                        "v2 stream: connection closed for agent {agent_id} \
                                         at seq {last_seq} — reconnecting"
                                    );
                                    break;
                                }
                                Some(Err(e)) => {
                                    tracing::warn!(
                                        "v2 stream: read error for agent {agent_id}: {e}"
                                    );
                                    break;
                                }
                                // Ping/Pong/Binary/Frame: ignore, keep reading.
                                Some(Ok(_)) => continue,
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("v2 stream: connect failed for agent {agent_id}: {e}");
                }
            }

            // Backoff before the next attempt. Capped at 5s so a server-side
            // restart is picked up promptly without hammering.
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max_backoff);
        }
    });

    (rx, handle.abort_handle())
}
