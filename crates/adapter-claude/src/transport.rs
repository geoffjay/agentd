//! AAP transport bindings for the Claude adapter.
//!
//! Both bindings are reduced to the same shape: a channel of inbound host
//! lines and a channel of outbound agent lines. The core loop is transport
//! agnostic and just moves [`agentd_agent_protocol`] messages across these.

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// A connected AAP transport: inbound host lines and an outbound line sender.
pub struct Transport {
    /// Lines received from the host (one JSON object each, no trailing newline).
    pub inbound: mpsc::UnboundedReceiver<String>,
    /// Send a line to the host (the transport appends framing as needed).
    pub outbound: mpsc::UnboundedSender<String>,
}

/// Which transport binding the host selected.
pub enum Binding {
    Stdio,
    Websocket(String),
}

impl Binding {
    /// Resolve the binding from the AAP transport environment variables.
    pub fn from_env() -> Result<Self> {
        let t = std::env::var(agentd_agent_protocol::ENV_TRANSPORT)
            .unwrap_or_else(|_| agentd_agent_protocol::TRANSPORT_STDIO.to_string());
        match t.as_str() {
            agentd_agent_protocol::TRANSPORT_STDIO => Ok(Binding::Stdio),
            agentd_agent_protocol::TRANSPORT_WEBSOCKET => {
                let url = std::env::var(agentd_agent_protocol::ENV_WS_URL)
                    .context("AGENTD_AAP_WS_URL is required for the websocket transport")?;
                Ok(Binding::Websocket(url))
            }
            other => anyhow::bail!("unknown AAP transport: {other}"),
        }
    }
}

/// Connect the selected transport and return the line channels.
pub async fn connect(binding: Binding) -> Result<Transport> {
    match binding {
        Binding::Stdio => Ok(connect_stdio()),
        Binding::Websocket(url) => connect_websocket(&url).await,
    }
}

fn connect_stdio() -> Transport {
    let (in_tx, in_rx) = mpsc::unbounded_channel::<String>();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    // Reader: host frames arrive on our stdin.
    tokio::spawn(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if in_tx.send(trimmed.to_string()).is_err() {
                break;
            }
        }
    });

    // Writer: agent frames go to our stdout, one NDJSON line each.
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(mut line) = out_rx.recv().await {
            line.push('\n');
            if stdout.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    Transport { inbound: in_rx, outbound: out_tx }
}

async fn connect_websocket(url: &str) -> Result<Transport> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let (ws, _resp) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("failed to connect AAP websocket at {url}"))?;
    let (mut write, mut read) = ws.split();

    let (in_tx, in_rx) = mpsc::unbounded_channel::<String>();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if in_tx.send(trimmed.to_string()).is_err() {
                        return;
                    }
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(line) = out_rx.recv().await {
            if write.send(Message::Text(line.into())).await.is_err() {
                break;
            }
        }
    });

    Ok(Transport { inbound: in_rx, outbound: out_tx })
}
