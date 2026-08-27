//! `agentd-adapter-claude` — the reference AAP adapter for Claude Code.
//!
//! Reads AAP over the transport the host selects (stdio or websocket), spawns
//! `claude` configured from the `initialize` message, and translates between
//! Claude's `stream-json` protocol and AAP. Claude holds no special status in
//! agentd; this binary is simply one implementation of
//! `docs/spec/agent-protocol-v1.md`.

mod claude;
mod translate;
mod transport;

use std::collections::HashMap;

use agentd_agent_protocol::{
    capability, ActivityState, AgentInfo, AgentMessage, HostMessage, PROTOCOL_VERSION,
};
use serde_json::Value;
use tracing::{info, warn};

/// Capabilities this adapter honestly supports today.
fn capabilities() -> Vec<String> {
    [
        capability::STREAMING,
        capability::THINKING,
        capability::TOOL_APPROVAL,
        capability::USAGE_REPORTING,
        capability::COST_REPORTING,
        capability::CANCEL,
        capability::MCP,
        capability::SYSTEM_PROMPT_APPEND,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs go to stderr; stdout is reserved for AAP frames (stdio binding).
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let binding = transport::Binding::from_env()?;
    let mut transport = transport::connect(binding).await?;

    // --- Handshake: first host message must be `initialize`. ---
    let params = loop {
        let Some(line) = transport.inbound.recv().await else {
            warn!("host closed before initialize");
            return Ok(());
        };
        match serde_json::from_str::<HostMessage>(&line) {
            Ok(HostMessage::Initialize(p)) => break p,
            Ok(_) => warn!("ignoring host message received before initialize"),
            Err(e) => warn!(%e, "failed to parse host message before initialize"),
        }
    };

    if params.protocol_version != PROTOCOL_VERSION {
        emit(
            &transport,
            AgentMessage::Error {
                fatal: true,
                code: Some("unsupported_protocol_version".into()),
                message: format!(
                    "adapter speaks AAP v{PROTOCOL_VERSION}, host requested v{}",
                    params.protocol_version
                ),
            },
        );
        return Ok(());
    }

    // --- Spawn claude configured from the initialize params. ---
    let mcp_path = claude::write_mcp_config(&params)?;
    let args = claude::build_args(&params, mcp_path.as_deref());
    let cwd = params.workspace.cwd.clone();
    let mut claude = match claude::spawn(&args, &cwd) {
        Ok(c) => c,
        Err(e) => {
            emit(
                &transport,
                AgentMessage::Error {
                    fatal: true,
                    code: Some("spawn_failed".into()),
                    message: e.to_string(),
                },
            );
            return Ok(());
        }
    };
    // Take sole ownership of claude's stdin so dropping it later sends EOF.
    let mut claude_stdin = claude.take_stdin();
    // Claude routes permission checks over stdio only after this handshake.
    if let Some(tx) = &claude_stdin {
        let _ = tx.send(claude::initialize_line());
    }
    info!("claude spawned; sent SDK initialize handshake");

    // --- Ready. ---
    emit(
        &transport,
        AgentMessage::Ready {
            protocol_version: PROTOCOL_VERSION,
            agent: AgentInfo {
                name: "claude-code".into(),
                version: option_env!("CARGO_PKG_VERSION").map(|s| s.to_string()),
            },
            capabilities: capabilities(),
            models: None,
        },
    );

    // --- Main loop. ---
    let mut current_turn = String::new();
    // request_id -> original tool input, so `allow` can supply `updatedInput`.
    let mut pending_inputs: HashMap<String, Value> = HashMap::new();

    loop {
        tokio::select! {
            host = transport.inbound.recv() => {
                let Some(line) = host else {
                    info!("host closed connection; shutting down");
                    break;
                };
                let msg = match serde_json::from_str::<HostMessage>(&line) {
                    Ok(m) => m,
                    Err(e) => { warn!(%e, "unparseable host message; skipping"); continue; }
                };
                match msg {
                    HostMessage::Prompt { turn_id, content } => {
                        current_turn = turn_id;
                        emit(&transport, AgentMessage::Status { state: ActivityState::Busy });
                        if let Some(tx) = &claude_stdin {
                            let _ = tx.send(translate::prompt_to_claude(&content.as_text()));
                        }
                    }
                    HostMessage::ApprovalResponse { request_id, decision, updated_input, message } => {
                        let input = updated_input
                            .or_else(|| pending_inputs.remove(&request_id))
                            .unwrap_or(Value::Null);
                        if let Some(tx) = &claude_stdin {
                            let _ = tx.send(translate::approval_to_claude(
                                &request_id, decision, &input, message.as_deref(),
                            ));
                        }
                    }
                    HostMessage::Cancel { .. } => {
                        // Best-effort interrupt of the active turn.
                        if let Some(tx) = &claude_stdin {
                            let _ = tx.send(serde_json::json!({
                                "type":"control_request",
                                "request_id":"interrupt",
                                "request":{"subtype":"interrupt"}
                            }).to_string());
                        }
                    }
                    HostMessage::ClearContext => {
                        warn!("clear_context not supported by this adapter; ignoring");
                    }
                    HostMessage::Shutdown => {
                        info!("shutdown requested");
                        drop(claude_stdin.take()); // Drop -> EOF to claude stdin.
                        break;
                    }
                    HostMessage::Initialize(_) => {
                        warn!("duplicate initialize; ignoring");
                    }
                }
            }
            out = claude.stdout.recv() => {
                let Some(line) = out else {
                    info!("claude stdout closed; agent exited");
                    break;
                };
                let translated = translate::claude_line(&line, &current_turn);
                for m in translated.messages {
                    // Cache the tool input so a later `allow` can echo it back.
                    if let AgentMessage::ApprovalRequest { request_id, input, .. } = &m {
                        pending_inputs.insert(request_id.clone(), input.clone());
                    }
                    emit(&transport, m);
                }
            }
        }
    }

    // Give claude a moment to exit cleanly after EOF, then ensure it's gone.
    claude.kill().await;
    let _ = claude.wait().await;
    Ok(())
}

/// Serialize and send an AAP agent message to the host.
fn emit(transport: &transport::Transport, msg: AgentMessage) {
    match msg.to_ndjson() {
        Ok(line) => {
            let _ = transport.outbound.send(line);
        }
        Err(e) => warn!(%e, "failed to serialize agent message"),
    }
}
