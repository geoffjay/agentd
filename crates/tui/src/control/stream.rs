use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// Spawns a background task that streams v2 conversation frames from the
/// orchestrator WebSocket `/v2/stream/{agent_id}` endpoint.
///
/// Frame shapes:
/// - `{"frame":"snapshot_begin","cursor":N,"agent_id":...}`
/// - `{"frame":"event","seq":N,"type":"agent:output",...}` (in both snapshot
///   and live phases — identical shape)
/// - `{"frame":"snapshot_end","seq":N}`
/// - `{"frame":"gap","skipped":N,"reason":"broadcast_lagged"}`
/// - `{"frame":"error","code":"...","message":"..."}`
///
/// The caller threads `since_seq` from its last observed `event` frame so a
/// reconnect replays only the delta. `0` means "fresh subscriber, send full
/// history".
pub fn spawn(
    base_url: &str,
    agent_id: Uuid,
    since_seq: i64,
) -> (mpsc::UnboundedReceiver<serde_json::Value>, tokio::task::AbortHandle) {
    let (tx, rx) = mpsc::unbounded_channel();

    let ws_url = base_url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let url = format!("{ws_url}/v2/stream/{agent_id}");

    let handle = tokio::spawn(async move {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                let (mut write, mut read) = ws_stream.split();
                let subscribe = serde_json::json!({
                    "frame": "subscribe",
                    "since_seq": since_seq,
                });
                if let Err(e) = write.send(Message::Text(subscribe.to_string().into())).await {
                    tracing::warn!("v2 stream subscribe failed for agent {agent_id}: {e}");
                    return;
                }
                while let Some(Ok(msg)) = read.next().await {
                    if let Message::Text(text) = msg {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            if tx.send(value).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("v2 stream WebSocket failed for agent {agent_id}: {e}");
            }
        }
    });

    (rx, handle.abort_handle())
}
