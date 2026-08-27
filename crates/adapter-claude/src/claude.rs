//! Spawning and configuring the native `claude` process.
//!
//! This is the Claude-specific knowledge that used to live in the orchestrator
//! (`build_claude_command`, `write_mcp_config`, `make_initialize_line`). The
//! adapter builds `claude`'s argv from AAP [`InitializeParams`], writes the
//! per-agent MCP config file, and exposes claude's stdio as line channels.

use std::io::Write as _;
use std::process::Stdio;

use agentd_agent_protocol::{InitializeParams, SystemPromptMode};
use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// The binary the adapter execs. Overridable for testing via `AGENTD_CLAUDE_BIN`.
fn claude_bin() -> String {
    std::env::var("AGENTD_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
}

/// A spawned claude process with its stdio reduced to line channels.
pub struct Claude {
    child: tokio::process::Child,
    /// Send raw NDJSON lines (no trailing newline) to claude's stdin.
    ///
    /// Wrapped in `Option` and handed to the caller via [`Claude::take_stdin`]
    /// so that dropping every sender closes claude's stdin (EOF). The struct
    /// must not retain a sender after the caller takes it, or shutdown hangs.
    stdin: Option<mpsc::UnboundedSender<String>>,
    /// Receive raw NDJSON lines from claude's stdout.
    pub stdout: mpsc::UnboundedReceiver<String>,
}

impl Claude {
    /// Take sole ownership of claude's stdin sender. Once the returned sender
    /// (and any clones) is dropped, claude receives EOF on stdin.
    pub fn take_stdin(&mut self) -> Option<mpsc::UnboundedSender<String>> {
        self.stdin.take()
    }

    /// Wait for the child to exit (used during shutdown).
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        Ok(self.child.wait().await?)
    }

    /// Best-effort kill of the child process.
    pub async fn kill(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Build claude's argv (excluding the binary) from AAP initialize params.
///
/// The adapter always speaks to claude over stdio pipes, so it uses claude's
/// subprocess stdio protocol flags. No shell is involved — arguments are passed
/// directly, so no shell escaping is required.
pub fn build_args(params: &InitializeParams, mcp_config_path: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--print".into(),
        "--verbose".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--permission-prompt-tool".into(),
        "stdio".into(),
    ];

    if let Some(model) = &params.model {
        args.push("--model".into());
        args.push(model.clone());
    }

    if params.workspace.worktree {
        args.push("--worktree".into());
    }

    for dir in &params.workspace.additional_dirs {
        args.push("--add-dir".into());
        args.push(dir.clone());
    }

    if let Some(path) = mcp_config_path {
        args.push("--mcp-config".into());
        args.push(path.to_string());
        args.push("--strict-mcp-config".into());
    }

    if let Some(sp) = &params.system_prompt {
        let (inline_flag, file_flag) = match sp.mode {
            SystemPromptMode::Replace => ("--system-prompt", "--system-prompt-file"),
            SystemPromptMode::Append => ("--append-system-prompt", "--append-system-prompt-file"),
        };
        if let Some(text) = &sp.text {
            args.push(inline_flag.into());
            args.push(text.clone());
        } else if let Some(path) = &sp.path {
            args.push(file_flag.into());
            args.push(path.clone());
        }
    }

    args
}

/// Write the per-agent MCP config file (`{ "mcpServers": {...} }`) with mode
/// 0600 and return its path, or `None` if there are no servers to configure.
pub fn write_mcp_config(params: &InitializeParams) -> Result<Option<String>> {
    let servers = match &params.tools {
        Some(t) if !t.mcp_servers.is_empty() => &t.mcp_servers,
        _ => return Ok(None),
    };

    let body = serde_json::json!({ "mcpServers": servers });
    let dir = std::env::temp_dir();
    let path = dir.join(format!("agentd-adapter-claude-{}.json", std::process::id()));

    let mut file = std::fs::File::create(&path)
        .with_context(|| format!("failed to create MCP config at {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    file.write_all(serde_json::to_string(&body)?.as_bytes())?;

    Ok(Some(path.to_string_lossy().into_owned()))
}

/// The claude SDK `initialize` control_request line. Claude only routes
/// permission checks over stdio after receiving this handshake; without it,
/// `--permission-prompt-tool stdio` is ignored and headless mode auto-denies.
pub fn initialize_line() -> String {
    serde_json::json!({
        "type": "control_request",
        "request_id": "init-adapter",
        "request": { "subtype": "initialize", "hooks": null },
    })
    .to_string()
}

/// Spawn `claude` with the given argv in `cwd`, returning line channels for its
/// stdin/stdout. stderr is inherited so claude's logs surface on the adapter's
/// stderr (the AAP-reserved log stream).
pub fn spawn(args: &[String], cwd: &str) -> Result<Claude> {
    let mut cmd = Command::new(claude_bin());
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut child = cmd.spawn().context("failed to spawn claude")?;

    let mut child_stdin = child.stdin.take().context("claude stdin missing")?;
    let child_stdout = child.stdout.take().context("claude stdout missing")?;

    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
    let (stdout_tx, stdout_rx) = mpsc::unbounded_channel::<String>();

    // Relay: our channel -> claude stdin (each line gets a trailing newline).
    tokio::spawn(async move {
        while let Some(mut line) = stdin_rx.recv().await {
            line.push('\n');
            if child_stdin.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            let _ = child_stdin.flush().await;
        }
        // Dropping child_stdin closes claude's stdin (EOF) for graceful exit.
    });

    // Reader: claude stdout NDJSON lines -> our channel.
    tokio::spawn(async move {
        let mut lines = BufReader::new(child_stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            if stdout_tx.send(line).is_err() {
                break;
            }
        }
    });

    Ok(Claude { child, stdin: Some(stdin_tx), stdout: stdout_rx })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_agent_protocol::{McpServer, SystemPrompt, Tools, Workspace};
    use std::collections::HashMap;

    fn params() -> InitializeParams {
        InitializeParams {
            protocol_version: 1,
            model: Some("claude-sonnet-5".into()),
            system_prompt: Some(SystemPrompt {
                mode: SystemPromptMode::Append,
                text: Some("be terse".into()),
                path: None,
            }),
            workspace: Workspace {
                cwd: "/repo".into(),
                additional_dirs: vec!["/extra".into()],
                worktree: true,
            },
            tools: None,
            resume_token: None,
        }
    }

    #[test]
    fn args_include_stdio_protocol_flags() {
        let args = build_args(&params(), None);
        assert!(args.windows(2).any(|w| w == ["--output-format", "stream-json"]));
        assert!(args.windows(2).any(|w| w == ["--input-format", "stream-json"]));
        assert!(args.windows(2).any(|w| w == ["--permission-prompt-tool", "stdio"]));
        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
    }

    #[test]
    fn args_map_config_fields() {
        let args = build_args(&params(), Some("/tmp/mcp.json"));
        assert!(args.windows(2).any(|w| w == ["--model", "claude-sonnet-5"]));
        assert!(args.contains(&"--worktree".to_string()));
        assert!(args.windows(2).any(|w| w == ["--add-dir", "/extra"]));
        assert!(args.windows(2).any(|w| w == ["--mcp-config", "/tmp/mcp.json"]));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        // append mode selects the append flag, not the replace flag.
        assert!(args.windows(2).any(|w| w == ["--append-system-prompt", "be terse"]));
        assert!(!args.iter().any(|a| a == "--system-prompt"));
    }

    #[test]
    fn mcp_config_written_only_when_servers_present() {
        assert!(write_mcp_config(&params()).unwrap().is_none());

        let mut p = params();
        let mut servers = HashMap::new();
        servers.insert(
            "agentd".to_string(),
            McpServer { command: "agent".into(), args: vec!["mcp".into()], env: HashMap::new() },
        );
        p.tools = Some(Tools { mcp_servers: servers });
        let path = write_mcp_config(&p).unwrap().expect("path");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("mcpServers"));
        assert!(contents.contains("agentd"));
        let _ = std::fs::remove_file(&path);
    }
}
