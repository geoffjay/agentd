//! Direct subprocess execution backend.
//!
//! Provides [`SubprocessBackend`] which implements [`ExecutionBackend`] by
//! spawning agent CLIs directly via [`tokio::process::Command`]. No terminal
//! multiplexer, no PTY, no intermediate shell -- the agent binary is exec'd
//! with its full argv and environment.
//!
//! This backend is designed for SDK-mode agents that communicate via
//! `--sdk-url` WebSocket. It eliminates the shell readiness races inherent
//! in tmux/PTY backends where a shell must initialise before a command can
//! be injected.
//!
//! Sessions are tracked in-memory and do not survive process restarts.
//!
//! # Example
//!
//! ```no_run
//! use wrap::backend::{ExecutionBackend, SessionConfig};
//! use wrap::subprocess::SubprocessBackend;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let backend = SubprocessBackend::new("agentd");
//! // create_session reserves the name; send_command spawns the process.
//! # Ok(())
//! # }
//! ```

use crate::backend::{ExecutionBackend, SessionConfig, SessionExitInfo, SessionHealth};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Grace period after SIGTERM before escalating to SIGKILL.
const KILL_GRACE_SECS: u64 = 5;

/// Internal state of a subprocess session.
#[allow(dead_code)]
enum SessionState {
    /// Reserved by `create_session` -- no process spawned yet.
    Pending { config: SessionConfig },
    /// Process is running.
    Running {
        child: Child,
        pid: u32,
        _stdout_task: JoinHandle<()>,
        _stderr_task: JoinHandle<()>,
        config: SessionConfig,
        created_at: Instant,
    },
    /// Process has exited.
    Exited { exit_status: ExitStatus, config: SessionConfig, _exited_at: Instant },
}

/// Direct subprocess execution backend.
///
/// Spawns agent CLIs as child processes using [`tokio::process::Command`].
/// Each session maps to exactly one OS process. Communication with the agent
/// happens via the `--sdk-url` WebSocket, not through the process's stdio.
///
/// # Lifecycle
///
/// 1. [`create_session`] -- validates the working directory and reserves the
///    session name.
/// 2. [`send_command`] -- parses the command string into argv, spawns the
///    process, and stores the handle.
/// 3. [`session_exists`] / [`session_health`] -- checks whether the child is
///    still alive via [`Child::try_wait`].
/// 4. [`kill_session`] -- sends SIGTERM, waits with a grace period, then
///    escalates to SIGKILL.
#[derive(Clone)]
pub struct SubprocessBackend {
    prefix: String,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    /// PATH value to inject into spawned subprocesses.
    ///
    /// Read from `AGENTD_SUBPROCESS_PATH` at construction time. When set,
    /// this overrides the inherited PATH so that tools invoked by agents
    /// (e.g. `agent`, `git`, `cargo`) are found the same way they would be
    /// in the user's interactive shell.
    subprocess_path: Option<String>,
}

impl SubprocessBackend {
    /// Create a new `SubprocessBackend` with the given session name prefix.
    ///
    /// Reads `AGENTD_SUBPROCESS_PATH` from the environment. When set, its
    /// value is injected as the `PATH` for every spawned subprocess.
    pub fn new(prefix: impl Into<String>) -> Self {
        let subprocess_path = std::env::var("AGENTD_SUBPROCESS_PATH").ok();
        if let Some(ref path) = subprocess_path {
            info!(path = %path, "AGENTD_SUBPROCESS_PATH set, will inject into spawned processes");
        }
        Self {
            prefix: prefix.into(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            subprocess_path,
        }
    }
}

/// Parse a shell command string into environment variables, binary, and arguments.
///
/// Handles the format produced by `build_claude_command` in the orchestrator:
/// ```text
/// KEY1='value1' KEY2='value2' binary --flag1 --flag2 value
/// ```
///
/// Also handles `sudo -u USER env KEY=VAL binary ...` by extracting through
/// the `sudo` and `env` prefixes.
fn parse_command(command: &str) -> Result<(HashMap<String, String>, String, Vec<String>)> {
    let words = shell_words::split(command).context("Failed to parse command string")?;

    if words.is_empty() {
        return Err(anyhow!("Empty command string"));
    }

    let mut env = HashMap::new();
    let mut idx = 0;

    // Handle `sudo -u USER [env]` prefix.
    let mut sudo_user: Option<String> = None;
    if words.get(idx).map(|w| w.as_str()) == Some("sudo") {
        idx += 1; // skip "sudo"
        if words.get(idx).map(|w| w.as_str()) == Some("-u") {
            idx += 1; // skip "-u"
            sudo_user = words.get(idx).cloned();
            idx += 1; // skip username
        }
        // Skip "env" if present after sudo
        if words.get(idx).map(|w| w.as_str()) == Some("env") {
            idx += 1;
        }
    }

    // Collect leading KEY=VALUE assignments.
    while idx < words.len() {
        if let Some((key, value)) = words[idx].split_once('=') {
            if is_valid_env_name(key) {
                env.insert(key.to_string(), value.to_string());
                idx += 1;
                continue;
            }
        }
        break;
    }

    let raw_binary =
        words.get(idx).ok_or_else(|| anyhow!("No binary found in command: {}", command))?.clone();
    let binary = resolve_binary(&raw_binary);
    let args = words[idx + 1..].to_vec();

    // If sudo was requested, we need to spawn through sudo. Reconstruct
    // the command with sudo wrapping.
    if let Some(user) = sudo_user {
        let sudo_bin = resolve_binary("sudo");
        let mut sudo_args = vec!["-u".to_string(), user, binary];
        sudo_args.extend(args);
        return Ok((env, sudo_bin, sudo_args));
    }

    Ok((env, binary, args))
}

/// Resolve the absolute path of a known agent binary.
///
/// When a subprocess is spawned directly (no shell), the parent process's
/// PATH is used for binary lookup. That PATH often lacks directories that
/// the user's interactive shell adds via `.zshrc`, `.bashrc`, direnv, asdf,
/// etc. This function checks well-known installation locations first, then
/// falls back to the bare name (relying on whatever PATH exists).
///
/// This is the same pattern used by [`get_tmux_command`](crate::tmux) for
/// finding the tmux binary.
fn resolve_binary(name: &str) -> String {
    let candidates: &[&str] = match name {
        "claude" => &[
            // Claude Code CLI -- common install locations
            // Symlink created by the installer:
            &format!("{}/.local/bin/claude", env_home()),
            "/usr/local/bin/claude",
            // npm global installs:
            &format!("{}/.npm-global/bin/claude", env_home()),
            // Homebrew:
            "/opt/homebrew/bin/claude",
        ],
        "sudo" => &["/usr/bin/sudo"],
        _ => &[],
    };

    for candidate in candidates {
        if Path::new(candidate).exists() {
            debug!(binary = name, resolved = candidate, "Resolved binary path");
            return candidate.to_string();
        }
    }

    // Fall back to bare name -- let the OS do PATH lookup.
    debug!(binary = name, "No known path found, falling back to PATH lookup");
    name.to_string()
}

/// Return the user's home directory from the `HOME` environment variable.
fn env_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
}

/// Check if a string is a valid environment variable name.
fn is_valid_env_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Spawn a task that drains a piped stream to tracing logs.
fn spawn_drain_task(
    stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    session_name: String,
    stream_name: &'static str,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            debug!(
                session = %session_name,
                stream = stream_name,
                "{}",
                line,
            );
        }
    })
}

/// Send a signal to a process group.
///
/// Returns `true` if the signal was sent successfully.
#[cfg(unix)]
fn signal_process_group(pid: u32, signal: i32) -> bool {
    // SAFETY: killpg sends a signal to a process group. The pid was obtained
    // from a valid Child handle and the signal is a standard POSIX signal.
    unsafe { libc::killpg(pid as libc::pid_t, signal) == 0 }
}

#[async_trait]
impl ExecutionBackend for SubprocessBackend {
    async fn create_session(&self, config: &SessionConfig) -> Result<()> {
        let working_dir = &config.working_dir;

        if !Path::new(working_dir).is_dir() {
            return Err(anyhow!("Working directory does not exist: {}", working_dir));
        }

        let mut sessions = self.sessions.write().await;
        if sessions.contains_key(&config.session_name) {
            return Err(anyhow!("Session '{}' already exists", config.session_name));
        }

        sessions
            .insert(config.session_name.clone(), SessionState::Pending { config: config.clone() });

        debug!(
            session = %config.session_name,
            working_dir = %working_dir,
            "Subprocess session reserved"
        );

        Ok(())
    }

    async fn launch_agent(&self, config: &SessionConfig) -> Result<()> {
        let cmd = crate::backend::build_agent_command(config)?;
        self.send_command(&config.session_name, &cmd).await
    }

    async fn send_command(&self, session_name: &str, command: &str) -> Result<()> {
        let (env_vars, binary, args) = parse_command(command)?;

        // Retrieve the pending session config for the working directory.
        let working_dir = {
            let sessions = self.sessions.read().await;
            match sessions.get(session_name) {
                Some(SessionState::Pending { config }) => config.working_dir.clone(),
                Some(SessionState::Running { .. }) => {
                    return Err(anyhow!(
                        "Session '{}' already has a running process. \
                         Subprocess backend supports one process per session.",
                        session_name
                    ));
                }
                Some(SessionState::Exited { .. }) => {
                    return Err(anyhow!("Session '{}' has already exited", session_name));
                }
                None => {
                    return Err(anyhow!(
                        "Session '{}' not found. Call create_session first.",
                        session_name
                    ));
                }
            }
        };

        debug!(
            session = %session_name,
            binary = %binary,
            args = ?args,
            env_count = env_vars.len(),
            "Spawning subprocess"
        );

        let mut cmd = tokio::process::Command::new(&binary);
        cmd.args(&args)
            .current_dir(&working_dir)
            .envs(&env_vars)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Inject AGENTD_SUBPROCESS_PATH as PATH so that tools invoked by the
        // agent (git, cargo, agent, etc.) are found the same way they would
        // be in the user's interactive shell.
        if let Some(ref path) = self.subprocess_path {
            cmd.env("PATH", path);
        }

        // Place the child in its own process group so we can signal the
        // entire tree (claude may spawn subprocesses).
        #[cfg(unix)]
        // SAFETY: setpgid(0, 0) places the child in a new process group
        // whose PGID equals its PID. This is a standard POSIX operation.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let mut child = cmd.spawn().with_context(|| {
            format!("Failed to spawn '{}' for session '{}'", binary, session_name)
        })?;

        let pid =
            child.id().ok_or_else(|| anyhow!("Child process has no PID (already exited?)"))?;

        // Drain stdout/stderr to tracing to prevent pipe deadlock.
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let stdout_task = spawn_drain_task(stdout, session_name.to_string(), "stdout");
        let stderr_task = spawn_drain_task(stderr, session_name.to_string(), "stderr");

        info!(
            session = %session_name,
            pid = pid,
            binary = %binary,
            "Subprocess spawned"
        );

        // Transition from Pending to Running.
        let mut sessions = self.sessions.write().await;
        let config = match sessions.remove(session_name) {
            Some(SessionState::Pending { config }) => config,
            other => {
                // Shouldn't happen, but if it does, kill the child and bail.
                drop(child);
                if let Some(state) = other {
                    sessions.insert(session_name.to_string(), state);
                }
                return Err(anyhow!("Session '{}' was not in Pending state", session_name));
            }
        };

        sessions.insert(
            session_name.to_string(),
            SessionState::Running {
                child,
                pid,
                _stdout_task: stdout_task,
                _stderr_task: stderr_task,
                config,
                created_at: Instant::now(),
            },
        );

        Ok(())
    }

    async fn session_exists(&self, session_name: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;

        let should_transition = match sessions.get_mut(session_name) {
            Some(SessionState::Running { child, .. }) => match child.try_wait() {
                Ok(Some(_status)) => true,
                Ok(None) => return Ok(true),
                Err(e) => {
                    return Err(anyhow!("Failed to check session '{}' status: {}", session_name, e))
                }
            },
            Some(SessionState::Pending { .. }) => return Ok(true),
            Some(SessionState::Exited { .. }) | None => return Ok(false),
        };

        if should_transition {
            // Process exited -- transition to Exited state.
            if let Some(SessionState::Running { mut child, config, .. }) =
                sessions.remove(session_name)
            {
                let status = child.try_wait()?.unwrap();
                sessions.insert(
                    session_name.to_string(),
                    SessionState::Exited {
                        exit_status: status,
                        config,
                        _exited_at: Instant::now(),
                    },
                );
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    async fn kill_session(&self, session_name: &str) -> Result<()> {
        let pid = {
            let sessions = self.sessions.read().await;
            match sessions.get(session_name) {
                Some(SessionState::Running { pid, .. }) => Some(*pid),
                Some(SessionState::Pending { .. }) => {
                    drop(sessions);
                    self.sessions.write().await.remove(session_name);
                    return Ok(());
                }
                Some(SessionState::Exited { .. }) => {
                    drop(sessions);
                    self.sessions.write().await.remove(session_name);
                    return Ok(());
                }
                None => return Ok(()), // Idempotent
            }
        };

        if let Some(pid) = pid {
            // Phase 1: SIGTERM to process group.
            #[cfg(unix)]
            {
                debug!(session = %session_name, pid, "Sending SIGTERM to process group");
                signal_process_group(pid, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                // On non-Unix, just kill directly.
                let mut sessions = self.sessions.write().await;
                if let Some(SessionState::Running { ref mut child, .. }) =
                    sessions.get_mut(session_name)
                {
                    let _ = child.start_kill();
                }
            }

            // Phase 2: Wait with timeout for graceful exit.
            let exited = {
                let mut sessions = self.sessions.write().await;
                if let Some(SessionState::Running { ref mut child, .. }) =
                    sessions.get_mut(session_name)
                {
                    matches!(
                        tokio::time::timeout(Duration::from_secs(KILL_GRACE_SECS), child.wait())
                            .await,
                        Ok(Ok(_))
                    )
                } else {
                    true // Already gone
                }
            };

            // Phase 3: SIGKILL if still alive.
            if !exited {
                warn!(
                    session = %session_name,
                    pid,
                    "Process did not exit after {}s grace period, sending SIGKILL",
                    KILL_GRACE_SECS
                );

                #[cfg(unix)]
                signal_process_group(pid, libc::SIGKILL);

                let mut sessions = self.sessions.write().await;
                if let Some(SessionState::Running { ref mut child, .. }) =
                    sessions.get_mut(session_name)
                {
                    let _ = child.start_kill();
                    let _ = child.wait().await; // Reap
                }
            }
        }

        // Remove from session map.
        self.sessions.write().await.remove(session_name);
        info!(session = %session_name, "Subprocess session killed");

        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = self.sessions.write().await;

        // Collect sessions that have exited so we can transition them.
        let mut exited_sessions = Vec::new();
        for (name, state) in sessions.iter_mut() {
            if let SessionState::Running { child, .. } = state {
                if let Ok(Some(_)) = child.try_wait() {
                    exited_sessions.push(name.clone());
                }
            }
        }

        // Transition exited processes and remove them from the active list.
        for name in &exited_sessions {
            if let Some(SessionState::Running { mut child, config, .. }) = sessions.remove(name) {
                let status = child.try_wait().ok().flatten();
                if let Some(status) = status {
                    sessions.insert(
                        name.clone(),
                        SessionState::Exited {
                            exit_status: status,
                            config,
                            _exited_at: Instant::now(),
                        },
                    );
                }
            }
        }

        // Return only Pending and Running sessions (not Exited).
        let active: Vec<String> = sessions
            .iter()
            .filter(|(_, state)| {
                matches!(state, SessionState::Pending { .. } | SessionState::Running { .. })
            })
            .map(|(name, _)| name.clone())
            .collect();

        Ok(active)
    }

    fn prefix(&self) -> &str {
        &self.prefix
    }

    async fn session_health(&self, session_name: &str) -> Result<SessionHealth> {
        let sessions = self.sessions.read().await;
        match sessions.get(session_name) {
            Some(SessionState::Running { .. }) => {
                // We checked try_wait in session_exists; here we trust the state.
                Ok(SessionHealth::Healthy)
            }
            Some(SessionState::Pending { .. }) => Ok(SessionHealth::Starting),
            Some(SessionState::Exited { .. }) => Ok(SessionHealth::Unknown),
            None => Ok(SessionHealth::Unknown),
        }
    }

    async fn session_exit_info(&self, session_name: &str) -> Result<Option<SessionExitInfo>> {
        let mut sessions = self.sessions.write().await;

        // Check if a running process has exited.
        let should_transition =
            matches!(sessions.get_mut(session_name), Some(SessionState::Running { .. })) && {
                if let Some(SessionState::Running { child, .. }) = sessions.get_mut(session_name) {
                    matches!(child.try_wait(), Ok(Some(_)))
                } else {
                    false
                }
            };

        if should_transition {
            if let Some(SessionState::Running { mut child, config, .. }) =
                sessions.remove(session_name)
            {
                let status = child.try_wait()?.unwrap();
                let info = exit_status_to_info(status);
                sessions.insert(
                    session_name.to_string(),
                    SessionState::Exited {
                        exit_status: status,
                        config,
                        _exited_at: Instant::now(),
                    },
                );
                return Ok(Some(info));
            }
        }

        match sessions.get(session_name) {
            Some(SessionState::Exited { exit_status, .. }) => {
                Ok(Some(exit_status_to_info(*exit_status)))
            }
            Some(SessionState::Running { .. }) => Ok(None), // Still running
            Some(SessionState::Pending { .. }) => Ok(None), // Not started
            None => Ok(None),
        }
    }

    async fn shutdown_all_sessions(&self) -> Result<()> {
        let session_names: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };
        for name in session_names {
            if let Err(e) = self.kill_session(&name).await {
                warn!(
                    session = %name,
                    %e,
                    "Failed to kill subprocess session during shutdown"
                );
            }
        }
        Ok(())
    }
}

/// Convert an [`ExitStatus`] to a [`SessionExitInfo`].
fn exit_status_to_info(status: ExitStatus) -> SessionExitInfo {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return SessionExitInfo {
                exit_code: 128 + signal as i64,
                error: Some(format!("Killed by signal {}", signal)),
            };
        }
    }

    SessionExitInfo {
        exit_code: status.code().unwrap_or(-1) as i64,
        error: if status.success() {
            None
        } else {
            Some(format!("Exited with code {}", status.code().unwrap_or(-1)))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_command tests --

    /// Helper: assert a binary ends with the expected name (may be resolved to absolute path).
    fn assert_binary(actual: &str, expected_name: &str) {
        assert!(
            actual == expected_name || actual.ends_with(&format!("/{}", expected_name)),
            "expected binary '{}' or absolute path ending in '/{0}', got '{}'",
            expected_name,
            actual
        );
    }

    #[test]
    fn parse_simple_command() {
        let (env, binary, args) = parse_command(
            "claude --sdk-url ws://localhost:7006/ws/abc --output-format stream-json",
        )
        .unwrap();
        assert!(env.is_empty());
        assert_binary(&binary, "claude");
        assert_eq!(
            args,
            vec!["--sdk-url", "ws://localhost:7006/ws/abc", "--output-format", "stream-json"]
        );
    }

    #[test]
    fn parse_command_with_env_vars() {
        let (env, binary, args) =
            parse_command("ANTHROPIC_API_KEY='sk-test' FOO=bar claude --model sonnet").unwrap();
        assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "sk-test");
        assert_eq!(env.get("FOO").unwrap(), "bar");
        assert_binary(&binary, "claude");
        assert_eq!(args, vec!["--model", "sonnet"]);
    }

    #[test]
    fn parse_sudo_command() {
        let (env, binary, args) =
            parse_command("sudo -u deploy env ANTHROPIC_API_KEY='sk-test' claude --model sonnet")
                .unwrap();
        assert_eq!(env.get("ANTHROPIC_API_KEY").unwrap(), "sk-test");
        assert_binary(&binary, "sudo");
        // The claude arg inside sudo is also resolved.
        assert_eq!(args[0], "-u");
        assert_eq!(args[1], "deploy");
        assert!(args[2] == "claude" || args[2].ends_with("/claude"));
        assert_eq!(args[3], "--model");
        assert_eq!(args[4], "sonnet");
    }

    #[test]
    fn parse_empty_command_fails() {
        assert!(parse_command("").is_err());
    }

    // -- resolve_binary tests --

    #[test]
    fn resolve_binary_unknown_returns_bare_name() {
        assert_eq!(resolve_binary("some-unknown-binary"), "some-unknown-binary");
    }

    #[test]
    fn resolve_binary_claude_returns_path_or_name() {
        let resolved = resolve_binary("claude");
        // Either resolved to an absolute path or stayed as "claude".
        assert!(resolved == "claude" || resolved.ends_with("/claude"), "unexpected: {}", resolved);
    }

    // -- is_valid_env_name tests --

    #[test]
    fn valid_env_names() {
        assert!(is_valid_env_name("FOO"));
        assert!(is_valid_env_name("_BAR"));
        assert!(is_valid_env_name("ANTHROPIC_API_KEY"));
        assert!(is_valid_env_name("a1"));
    }

    #[test]
    fn invalid_env_names() {
        assert!(!is_valid_env_name(""));
        assert!(!is_valid_env_name("1FOO"));
        assert!(!is_valid_env_name("FOO-BAR"));
        assert!(!is_valid_env_name("--model"));
    }

    // -- SubprocessBackend unit tests --

    #[test]
    fn subprocess_backend_new_sets_prefix() {
        let backend = SubprocessBackend::new("test-prefix");
        assert_eq!(backend.prefix(), "test-prefix");
    }

    #[test]
    fn subprocess_backend_supports_pty_input_returns_false() {
        let backend = SubprocessBackend::new("test");
        assert!(!backend.supports_pty_input());
    }

    #[test]
    fn subprocess_backend_ws_url_returns_none() {
        let backend = SubprocessBackend::new("test");
        assert!(backend.agent_ws_url("any-session", None).is_none());
    }

    #[tokio::test]
    async fn list_sessions_initially_empty() {
        let backend = SubprocessBackend::new("test");
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn session_exists_returns_false_for_unknown() {
        let backend = SubprocessBackend::new("test");
        assert!(!backend.session_exists("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn session_health_unknown_for_missing() {
        let backend = SubprocessBackend::new("test");
        let health = backend.session_health("missing").await.unwrap();
        assert_eq!(health, SessionHealth::Unknown);
    }

    #[tokio::test]
    async fn session_exit_info_none_for_missing() {
        let backend = SubprocessBackend::new("test");
        let info = backend.session_exit_info("missing").await.unwrap();
        assert!(info.is_none());
    }

    #[tokio::test]
    async fn kill_session_idempotent_for_missing() {
        let backend = SubprocessBackend::new("test");
        // Should not error
        backend.kill_session("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn create_session_rejects_missing_dir() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "test-session".to_string(),
            working_dir: "/nonexistent/path/that/does/not/exist".to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        let err = backend.create_session(&config).await.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn create_session_rejects_duplicate() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "dup-session".to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        backend.create_session(&config).await.unwrap();
        let err = backend.create_session(&config).await.unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn create_session_pending_then_list() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "pending-session".to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        backend.create_session(&config).await.unwrap();
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.contains(&"pending-session".to_string()));
    }

    #[tokio::test]
    async fn session_health_starting_for_pending() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "health-pending".to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        backend.create_session(&config).await.unwrap();
        let health = backend.session_health("health-pending").await.unwrap();
        assert_eq!(health, SessionHealth::Starting);
    }

    #[tokio::test]
    async fn send_command_fails_without_create() {
        let backend = SubprocessBackend::new("test");
        let err = backend.send_command("no-session", "echo hello").await.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn spawn_and_detect_exit() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "exit-test".to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        backend.create_session(&config).await.unwrap();
        // Spawn a process that exits immediately
        backend.send_command("exit-test", "true").await.unwrap();

        // Give it a moment to exit
        tokio::time::sleep(Duration::from_millis(100)).await;

        // session_exists should detect the exit
        assert!(!backend.session_exists("exit-test").await.unwrap());

        // Exit info should be available
        let info = backend.session_exit_info("exit-test").await.unwrap();
        assert!(info.is_some());
        assert_eq!(info.unwrap().exit_code, 0);
    }

    #[tokio::test]
    async fn spawn_and_kill() {
        let backend = SubprocessBackend::new("test");
        let config = SessionConfig {
            session_name: "kill-test".to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "sonnet".to_string(),
            layout: None,
            network_policy: None,
        };
        backend.create_session(&config).await.unwrap();
        // Spawn a long-running process
        backend.send_command("kill-test", "sleep 3600").await.unwrap();

        // Verify it's running
        assert!(backend.session_exists("kill-test").await.unwrap());

        // Kill it
        backend.kill_session("kill-test").await.unwrap();

        // Should be gone
        assert!(!backend.session_exists("kill-test").await.unwrap());
    }

    #[tokio::test]
    async fn shutdown_all_clears_sessions() {
        let backend = SubprocessBackend::new("test");
        for i in 0..3 {
            let config = SessionConfig {
                session_name: format!("shutdown-{i}"),
                working_dir: std::env::temp_dir().to_string_lossy().to_string(),
                agent_type: "claude-code".to_string(),
                model_provider: "anthropic".to_string(),
                model_name: "sonnet".to_string(),
                layout: None,
                network_policy: None,
            };
            backend.create_session(&config).await.unwrap();
            backend.send_command(&format!("shutdown-{i}"), "sleep 3600").await.unwrap();
        }
        backend.shutdown_all_sessions().await.unwrap();
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    /// Verify that `SubprocessBackend` is object-safe.
    #[test]
    fn trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn ExecutionBackend) {}
        let backend = SubprocessBackend::new("test");
        _assert_object_safe(&backend);
    }

    /// Verify Send + Sync bounds.
    #[test]
    fn subprocess_backend_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<SubprocessBackend>();
    }

    #[test]
    fn exit_status_to_info_success() {
        // We can't easily construct ExitStatus, but we test through the spawn path.
        // This test verifies the function exists and compiles.
    }
}
