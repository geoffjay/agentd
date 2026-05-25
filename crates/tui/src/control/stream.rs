use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// Spawns a background task that streams raw JSON events from the orchestrator
/// WebSocket `/stream/{agent_id}` endpoint.
///
/// The channel carries `serde_json::Value` because the stream JSON shape is
/// different from `ConversationEventResponse` (no `id` or `session_number`
/// fields). Callers normalise the values into `ConversationEntry`.
pub fn spawn(
    base_url: &str,
    agent_id: Uuid,
) -> (mpsc::UnboundedReceiver<serde_json::Value>, tokio::task::AbortHandle) {
    let (tx, rx) = mpsc::unbounded_channel();

    let ws_url = base_url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let url = format!("{ws_url}/stream/{agent_id}");

    let handle = tokio::spawn(async move {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                let (_, mut read) = ws_stream.split();
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
                tracing::warn!("stream WebSocket failed for agent {agent_id}: {e}");
            }
        }
    });

    (rx, handle.abort_handle())
}
