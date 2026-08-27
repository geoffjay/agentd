//! Wrap service command implementations.
//!
//! This module implements all subcommands for managing agent launches via the
//! wrap service. The wrap service supports three execution backends selected
//! with the `AGENTD_BACKEND` environment variable on the service side:
//!
//! | Backend | Description |
//! |---------|-------------|
//! | `tmux`  | Detached tmux sessions (default) |
//! | `pty`   | In-process PTY sessions — supports interactive attach |
//!
//! # Available Commands
//!
//! - **health**  — Check service health
//! - **info**    — Show active backend type and capabilities
//! - **list**    — List sessions with backend/capability columns
//! - **kill**    — Kill a session by name
//! - **launch**  — Launch an agent session
//! - **attach**  — Interactively attach to a PTY session (PTY backend only)
//!
//! # Examples
//!
//! ## Show backend info
//!
//! ```bash
//! agent wrap info
//! ```
//!
//! ## Launch a PTY session
//!
//! ```bash
//! agent wrap launch my-project \
//!   --path /path/to/project \
//!   --agent claude-code \
//!   --backend pty
//! ```
//!
//! ## Attach to a PTY session
//!
//! ```bash
//! agent wrap attach my-project
//! # Press Ctrl-D to detach
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use wrap::client::WrapClient;
use wrap::types::*;

/// Wrap service management subcommands.
#[derive(Subcommand)]
pub enum WrapCommand {
    /// Check the health of the wrap service.
    Health,

    /// Show the active execution backend type and capabilities.
    ///
    /// Queries the wrap service for its current backend configuration.
    Info,

    /// List all active sessions.
    ///
    /// Shows sessions managed by the wrap service with their backend type
    /// and supported capabilities.
    List,

    /// Kill a session by name.
    ///
    /// Terminates the specified session and any processes running in it.
    Kill {
        /// Session name to kill
        name: String,
    },

    /// Launch an agent session.
    ///
    /// Creates a new session and starts the specified agent with the given
    /// configuration. The session runs in the background.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Launch with default settings
    /// agent wrap launch my-project --path /home/user/projects/my-project
    ///
    /// # Launch with specific backend hint
    /// agent wrap launch my-project --path /home/user/project --backend pty
    ///
    /// # Launch with custom model
    /// agent wrap launch my-project \
    ///   --path /home/user/project \
    ///   --agent opencode \
    ///   --provider openai \
    ///   --model gpt-4
    /// ```
    Launch {
        /// Session name (required)
        session_name: String,

        /// Working directory path for the agent (defaults to current directory)
        #[arg(long, default_value = ".")]
        path: String,

        /// Agent type (claude-code, crush, opencode, gemini, etc.)
        #[arg(long)]
        agent: Option<String>,

        /// Model provider (anthropic, openai, ollama, etc.)
        #[arg(long)]
        provider: Option<String>,

        /// Model name (claude-sonnet-4.5, gpt-4, etc.)
        #[arg(long)]
        model: Option<String>,

        /// Optional tmux layout configuration as JSON string
        #[arg(long)]
        layout_json: Option<String>,

        /// Requested backend type (tmux, pty, subprocess).
        ///
        /// This is forwarded to the service as a hint. The service uses
        /// whatever backend is configured via `AGENTD_BACKEND` at startup;
        /// this flag does not override the service configuration.
        #[arg(long)]
        backend: Option<String>,
    },

    /// Interactively attach to a PTY session.
    ///
    /// Opens a raw terminal connected to the named session via the wrap
    /// service's WebSocket PTY relay. Only available when the wrap service
    /// is running with the PTY backend (`AGENTD_BACKEND=pty`).
    ///
    /// Press **Ctrl-D** to detach cleanly.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent wrap attach my-project
    /// ```
    Attach {
        /// Session name to attach to
        session: String,
    },
}

impl WrapCommand {
    /// Execute the wrap command.
    pub async fn execute(&self, client: &WrapClient, json: bool) -> Result<()> {
        match self {
            WrapCommand::Health => wrap_health(client, json).await,
            WrapCommand::Info => wrap_info(client, json).await,
            WrapCommand::List => list_sessions(client, json).await,
            WrapCommand::Kill { name } => kill_session(client, name, json).await,
            WrapCommand::Launch {
                session_name,
                path,
                agent,
                provider,
                model,
                layout_json,
                backend,
            } => {
                launch_agent(
                    client,
                    session_name,
                    path,
                    agent.as_deref(),
                    provider.as_deref(),
                    model.as_deref(),
                    layout_json.as_deref(),
                    backend.as_deref(),
                    json,
                )
                .await
            }
            WrapCommand::Attach { session } => attach_session(client, session).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn wrap_health(client: &WrapClient, json: bool) -> Result<()> {
    client.health().await.context("Failed to reach wrap service. Is it running?")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok"}))?);
    } else {
        println!("{} {}", "wrap:".bold(), "ok".green().bold());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Info
// ---------------------------------------------------------------------------

async fn wrap_info(client: &WrapClient, json: bool) -> Result<()> {
    let info = client.info().await.context("Failed to reach wrap service. Is it running?")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("{}", "Wrap Service Backend".blue().bold());
        println!("{}", "=".repeat(40).cyan());
        println!("{:<16} {}", "Backend:".bold(), info.backend_type.to_string().bright_white());
        println!("{:<16} {}", "Version:".bold(), info.version.bright_white());
        if info.capabilities.is_empty() {
            println!("{:<16} {}", "Capabilities:".bold(), "none".yellow());
        } else {
            println!(
                "{:<16} {}",
                "Capabilities:".bold(),
                info.capabilities.join(", ").bright_white()
            );
        }
        println!("{}", "=".repeat(40).cyan());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

async fn list_sessions(client: &WrapClient, json: bool) -> Result<()> {
    let response = client.list_sessions().await.context("Failed to list sessions")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.sessions.is_empty() {
        println!("{}", "No active sessions.".yellow());
    } else {
        // Column widths
        let name_w = response.sessions.iter().map(|s| s.name.len()).max().unwrap_or(4).max(4);
        let backend_w = 7usize;
        let status_w = 7usize;

        println!(
            "{:<name_w$}  {:<backend_w$}  {:<status_w$}  {}",
            "NAME".bold(),
            "BACKEND".bold(),
            "STATUS".bold(),
            "CAPABILITIES".bold(),
        );
        println!("{}", "-".repeat(name_w + backend_w + status_w + 20).cyan());

        for session in &response.sessions {
            let status = if session.active { "running".green() } else { "stopped".red() };
            let caps = if session.capabilities.is_empty() {
                "-".to_string()
            } else {
                session.capabilities.join(",")
            };
            println!(
                "{:<name_w$}  {:<backend_w$}  {:<status_w$}  {}",
                session.name.bright_white(),
                session.backend.to_string(),
                status,
                caps.bright_black(),
            );
        }

        println!();
        println!("Total: {} session(s)", response.count);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Kill
// ---------------------------------------------------------------------------

async fn kill_session(client: &WrapClient, name: &str, json: bool) -> Result<()> {
    let response =
        client.kill_session(name).await.context(format!("Failed to kill session '{}'", name))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.success {
        println!("{}", format!("Session '{}' terminated.", name).green().bold());
    } else {
        println!("{}", format!("Failed to kill session: {}", response.message).red());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn launch_agent(
    client: &WrapClient,
    session_name: &str,
    path: &str,
    agent: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
    layout_json: Option<&str>,
    backend: Option<&str>,
    json: bool,
) -> Result<()> {
    // Expand ~ and resolve to absolute path
    let expanded = if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            path.replacen('~', &home, 1)
        } else {
            path.to_string()
        }
    } else if path == "~" {
        std::env::var("HOME").unwrap_or_else(|_| path.to_string())
    } else {
        path.to_string()
    };

    let absolute_path = std::path::PathBuf::from(&expanded)
        .canonicalize()
        .context(format!("Failed to resolve project path: {expanded}"))?;
    let project_path = absolute_path.to_str().context("Invalid UTF-8 in project path")?.to_string();

    let layout_obj = if let Some(layout_str) = layout_json {
        Some(serde_json::from_str::<TmuxLayout>(layout_str).context("Invalid layout JSON")?)
    } else {
        None
    };

    let request = LaunchRequest {
        project_name: session_name.to_string(),
        project_path,
        agent_type: agent.unwrap_or("claude-code").to_string(),
        model_provider: provider.unwrap_or("anthropic").to_string(),
        model_name: model.unwrap_or("claude-sonnet-4.5").to_string(),
        layout: layout_obj,
        backend: backend.map(str::to_string),
    };

    let response = client.launch(&request).await.context("Failed to launch agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        if response.success {
            println!("{}", "Agent launched successfully!".green().bold());
        } else {
            println!("{}", "Agent launch failed!".red().bold());
        }
        println!();
        display_launch_response(&response);
    }

    Ok(())
}

fn display_launch_response(response: &LaunchResponse) {
    println!("{}", "=".repeat(80).cyan());
    println!(
        "{}: {}",
        "Status".bold(),
        if response.success { "Success".green() } else { "Failed".red() }
    );
    if let Some(ref name) = response.session_name {
        println!("{}: {}", "Session Name".bold(), name.bright_white());
    }
    println!("{}: {}", "Message".bold(), response.message);
    if let Some(ref error) = response.error {
        println!("{}: {}", "Error".bold().red(), error.bright_red());
    }
    println!("{}", "=".repeat(80).cyan());
}

// ---------------------------------------------------------------------------
// Attach
// ---------------------------------------------------------------------------

/// Attach to a PTY session using a raw terminal + WebSocket relay.
///
/// Verifies that:
/// 1. The wrap service is using the PTY backend.
/// 2. The named session exists.
///
/// Then opens the WebSocket terminal endpoint, enables crossterm raw mode,
/// and bridges stdin/stdout with the remote PTY until Ctrl-D is pressed or
/// the connection closes.
async fn attach_session(client: &WrapClient, session: &str) -> Result<()> {
    // 1. Verify PTY backend is active
    let info = client.info().await.context("Failed to reach wrap service. Is it running?")?;

    if info.backend_type != BackendType::Pty {
        anyhow::bail!(
            "Session attach requires the PTY backend.\n\
             Active backend: {}\n\
             Set AGENTD_BACKEND=pty on the wrap service to enable interactive attach.",
            info.backend_type
        );
    }

    // 2. Verify session exists
    client
        .get_session(session)
        .await
        .with_context(|| format!("Session '{}' not found", session))?;

    // 3. Build WebSocket URL
    let ws_url = client.terminal_ws_url(session);

    // 4. Connect to the PTY terminal relay
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .with_context(|| format!("Failed to connect to terminal at {ws_url}"))?;

    let (mut ws_tx, mut ws_rx) = futures_util::StreamExt::split(ws_stream);

    // 5. Enable raw terminal mode — disable_raw_mode on all exit paths via guard
    crossterm::terminal::enable_raw_mode().context("Failed to enable raw terminal mode")?;

    struct RawModeGuard;
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _guard = RawModeGuard;

    // Print detach hint before raw mode output begins
    eprintln!("Attached to '{}'. Press Ctrl-D to detach.\r", session);

    // Send initial terminal size so the server can size the PTY
    if let Ok((cols, rows)) = crossterm::terminal::size() {
        let resize = serde_json::json!({"type": "resize", "cols": cols, "rows": rows});
        let _ = ws_tx
            .send(tokio_tungstenite::tungstenite::Message::Text(resize.to_string().into()))
            .await;
    }

    // 6. I/O relay: stdin → WS (binary), WS → stdout (binary)
    use futures_util::SinkExt;
    use std::io::Write as _;
    use tokio::io::AsyncReadExt as _;

    let mut stdin = tokio::io::stdin();
    let mut stdout = std::io::stdout();
    let mut buf = [0u8; 1024];

    let result: Result<()> = loop {
        tokio::select! {
            // Terminal stdin → WebSocket
            n = stdin.read(&mut buf) => {
                match n {
                    Ok(0) => break Ok(()), // EOF
                    Ok(n) => {
                        let data = &buf[..n];
                        // Ctrl-D (\x04) detaches
                        if data.contains(&0x04) {
                            break Ok(());
                        }
                        if ws_tx
                            .send(tokio_tungstenite::tungstenite::Message::Binary(
                                bytes::Bytes::copy_from_slice(data),
                            ))
                            .await
                            .is_err()
                        {
                            break Ok(()); // WS closed
                        }
                    }
                    Err(e) => break Err(e.into()),
                }
            }
            // WebSocket → terminal stdout
            msg = futures_util::StreamExt::next(&mut ws_rx) => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Binary(data))) => {
                        stdout.write_all(&data)?;
                        stdout.flush()?;
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => {
                        break Ok(());
                    }
                    Some(Err(_)) => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    // Drop _guard here → disable_raw_mode runs, then print a clean newline
    drop(_guard);
    eprintln!("\r\nDetached from '{}'.", session);

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_launch_response_success() {
        let response = LaunchResponse {
            success: true,
            session_name: Some("test-session".to_string()),
            message: "Agent launched successfully".to_string(),
            error: None,
        };
        display_launch_response(&response);
    }

    #[test]
    fn test_display_launch_response_failure() {
        let response = LaunchResponse {
            success: false,
            session_name: None,
            message: "Failed to start session".to_string(),
            error: Some("Failed to start session".to_string()),
        };
        display_launch_response(&response);
    }

    #[test]
    fn backend_type_display() {
        assert_eq!(BackendType::Tmux.to_string(), "tmux");
        assert_eq!(BackendType::Pty.to_string(), "pty");
    }

    #[test]
    fn backend_type_capabilities() {
        assert!(BackendType::Pty.capabilities().contains(&"terminal".to_string()));
        assert!(BackendType::Pty.capabilities().contains(&"interactive".to_string()));
        assert!(BackendType::Tmux.capabilities().contains(&"attach-tmux".to_string()));
    }

    #[test]
    fn backend_type_serde_roundtrip() {
        for bt in [BackendType::Tmux, BackendType::Pty] {
            let json = serde_json::to_string(&bt).unwrap();
            let decoded: BackendType = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, bt);
        }
    }

    #[test]
    fn terminal_ws_url_converts_scheme() {
        let client = WrapClient::new("http://localhost:17005");
        assert_eq!(
            client.terminal_ws_url("my-session"),
            "ws://localhost:17005/sessions/my-session/terminal"
        );
    }

    #[test]
    fn terminal_ws_url_https_converts_to_wss() {
        let client = WrapClient::new("https://example.com:17005");
        assert_eq!(
            client.terminal_ws_url("my-session"),
            "wss://example.com:17005/sessions/my-session/terminal"
        );
    }
}
