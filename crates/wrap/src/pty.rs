//! PTY execution backend using `portable-pty`.
//!
//! Provides [`PtyBackend`] which implements [`ExecutionBackend`] using in-process
//! PTY sessions instead of tmux or Docker. Sessions are tracked in-memory and
//! are not persistent across process restarts.
//!
//! # Example
//! ```no_run
//! use wrap::backend::{ExecutionBackend, SessionConfig};
//! use wrap::pty::PtyBackend;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let backend = PtyBackend::new("agentd");
//! # Ok(())
//! # }
//! ```

use crate::backend::{
    build_agent_command, ExecutionBackend, SessionConfig, SessionExitInfo, SessionHealth,
};
use crate::pty_stream::{PtyOutputStream, DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Grace period given to a session after Ctrl-C before a hard kill is issued.
const KILL_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(200);

/// An active PTY session.
#[allow(dead_code)]
struct PtySession {
    /// The PTY master — retained so the PTY pair stays alive.
    master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
    /// The child process running in the PTY.
    child: Arc<tokio::sync::Mutex<Box<dyn Child + Send + Sync>>>,
    /// Combined output stream and input writer for this session.
    stream: PtyOutputStream,
    /// Working directory at session creation.
    working_dir: PathBuf,
    /// Original session config.
    config: SessionConfig,
    /// When the session was created.
    created_at: Instant,
}

/// PTY-based execution backend.
///
/// Manages agent sessions as in-process PTY pairs using [`portable_pty`].
/// No external binary dependencies (unlike [`TmuxBackend`][crate::backend::TmuxBackend]).
///
/// Sessions are stored in-memory; they do not survive process restarts.
#[derive(Clone)]
pub struct PtyBackend {
    prefix: String,
    sessions: Arc<RwLock<HashMap<String, PtySession>>>,
}

impl PtyBackend {
    /// Create a new `PtyBackend` with the given session name prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self { prefix: prefix.into(), sessions: Arc::new(RwLock::new(HashMap::new())) }
    }
}

#[async_trait]
impl ExecutionBackend for PtyBackend {
    async fn create_session(&self, config: &SessionConfig) -> Result<()> {
        let session_name = config.session_name.clone();
        let working_dir = PathBuf::from(&config.working_dir);
        let config_clone = config.clone();

        // Use spawn_blocking for the synchronous PTY API
        let (master, child, writer, reader) = tokio::task::spawn_blocking(move || {
            let pty_system = native_pty_system();
            let pair = pty_system
                .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
                .context("Failed to open PTY pair")?;

            // Build a simple shell command — agent is launched in launch_agent()
            let mut cmd = CommandBuilder::new_default_prog();
            cmd.cwd(&working_dir);

            let child = pair.slave.spawn_command(cmd).context("Failed to spawn shell in PTY")?;
            let writer = pair.master.take_writer().context("Failed to get PTY writer")?;
            let reader = pair.master.try_clone_reader().context("Failed to clone PTY reader")?;

            Ok::<_, anyhow::Error>((pair.master, child, writer, reader))
        })
        .await
        .context("spawn_blocking panicked")??;

        let stream = PtyOutputStream::new(DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES, writer);
        // Spawn the background reader task (moves a clone of the stream)
        stream.clone().spawn_reader(reader);

        let session = PtySession {
            master: Arc::new(std::sync::Mutex::new(master)),
            child: Arc::new(tokio::sync::Mutex::new(child)),
            stream,
            working_dir: PathBuf::from(&config_clone.working_dir),
            config: config_clone,
            created_at: Instant::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_name, session);
        Ok(())
    }

    async fn launch_agent(&self, config: &SessionConfig) -> Result<()> {
        let cmd = build_agent_command(config)?;
        self.send_command(&config.session_name, &cmd).await
    }

    async fn session_exists(&self, session_name: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;
        let Some(session) = sessions.get(session_name) else {
            return Ok(false);
        };
        // Check if the child process is still alive
        let mut child = session.child.lock().await;
        match child.try_wait() {
            Ok(Some(_)) => {
                // Process has exited — remove from map
                drop(child); // Release the mutex before removing
                sessions.remove(session_name);
                Ok(false)
            }
            Ok(None) => Ok(true), // Still running
            Err(e) => Err(anyhow!("Failed to check session '{}' status: {}", session_name, e)),
        }
    }

    async fn kill_session(&self, session_name: &str) -> Result<()> {
        // Phase 1: Send Ctrl-C gracefully via the output stream's writer
        {
            let sessions = self.sessions.read().await;
            let Some(session) = sessions.get(session_name) else {
                return Ok(());
            };
            let _ = session.stream.write_input(&[0x03]); // ETX = Ctrl-C
        } // Lock released here

        // Give the process a moment to exit gracefully
        tokio::time::sleep(KILL_GRACE_PERIOD).await;

        // Phase 2: Force kill and remove
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get(session_name) {
                let mut child = session.child.lock().await;
                let _ = child.kill();
            }
            sessions.remove(session_name);
        }
        Ok(())
    }

    async fn send_command(&self, session_name: &str, command: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_name)
            .ok_or_else(|| anyhow!("Session '{}' not found", session_name))?;

        let line = format!("{}\n", command);
        session
            .stream
            .write_input(line.as_bytes())
            .with_context(|| format!("Failed to write command to session '{}'", session_name))?;
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<String>> {
        // Upgrade to a write lock so we can prune sessions whose child
        // process has already exited without an explicit kill_session call.
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, session| {
            matches!(session.child.try_lock().ok().and_then(|mut c| c.try_wait().ok()), Some(None))
        });
        Ok(sessions.keys().cloned().collect())
    }

    fn prefix(&self) -> &str {
        &self.prefix
    }

    async fn resize_session(&self, session_name: &str, cols: u16, rows: u16) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_name) {
            let master = session
                .master
                .lock()
                .map_err(|_| anyhow!("PTY master lock poisoned for '{}'", session_name))?;
            master
                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                .with_context(|| format!("Failed to resize PTY session '{}'", session_name))?;
        }
        Ok(())
    }

    async fn session_output_stream(&self, session_name: &str) -> Result<Option<PtyOutputStream>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_name).map(|s| s.stream.clone()))
    }

    fn supports_pty_input(&self) -> bool {
        true
    }

    async fn session_health(&self, session_name: &str) -> Result<SessionHealth> {
        let sessions = self.sessions.read().await;
        let Some(session) = sessions.get(session_name) else {
            return Ok(SessionHealth::Unknown);
        };
        let mut child = session.child.lock().await;
        match child.try_wait() {
            Ok(None) => Ok(SessionHealth::Healthy),    // Still running
            Ok(Some(_)) => Ok(SessionHealth::Unknown), // Exited
            Err(_) => Ok(SessionHealth::Unknown),
        }
    }

    async fn session_exit_info(&self, session_name: &str) -> Result<Option<SessionExitInfo>> {
        let sessions = self.sessions.read().await;
        let Some(session) = sessions.get(session_name) else {
            return Ok(None);
        };
        let mut child = session.child.lock().await;
        match child.try_wait() {
            Ok(Some(exit_status)) => {
                Ok(Some(SessionExitInfo { exit_code: exit_status.exit_code() as i64, error: None }))
            }
            Ok(None) => Ok(None), // Still running
            Err(e) => Err(anyhow!("Failed to get exit info for '{}': {}", session_name, e)),
        }
    }

    async fn shutdown_all_sessions(&self) -> Result<()> {
        let session_names: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };
        for name in session_names {
            if let Err(e) = self.kill_session(&name).await {
                tracing::warn!("Failed to kill PTY session '{}' during shutdown: {}", name, e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str) -> SessionConfig {
        SessionConfig {
            session_name: name.to_string(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "claude-sonnet-4.5".to_string(),
            layout: None,
            network_policy: None,
        }
    }

    #[test]
    fn pty_backend_new_sets_prefix() {
        let backend = PtyBackend::new("test-prefix");
        assert_eq!(backend.prefix(), "test-prefix");
    }

    #[test]
    fn pty_backend_supports_pty_input() {
        let backend = PtyBackend::new("test");
        assert!(backend.supports_pty_input());
    }

    #[test]
    fn pty_backend_ws_url_returns_none() {
        let backend = PtyBackend::new("test");
        assert!(backend.agent_ws_url("any-session", None).is_none());
    }

    #[tokio::test]
    async fn list_sessions_initially_empty() {
        let backend = PtyBackend::new("test");
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn session_exists_returns_false_for_unknown() {
        let backend = PtyBackend::new("test");
        assert!(!backend.session_exists("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn session_health_unknown_for_missing() {
        let backend = PtyBackend::new("test");
        let health = backend.session_health("missing").await.unwrap();
        assert_eq!(health, SessionHealth::Unknown);
    }

    #[tokio::test]
    async fn session_exit_info_none_for_missing() {
        let backend = PtyBackend::new("test");
        let info = backend.session_exit_info("missing").await.unwrap();
        assert!(info.is_none());
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn create_and_list_session() {
        let backend = PtyBackend::new("test");
        let config = test_config("test-session");
        backend.create_session(&config).await.unwrap();
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.contains(&"test-session".to_string()));
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn session_exists_after_create() {
        let backend = PtyBackend::new("test");
        let config = test_config("exists-session");
        backend.create_session(&config).await.unwrap();
        assert!(backend.session_exists("exists-session").await.unwrap());
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn kill_session_removes_from_list() {
        let backend = PtyBackend::new("test");
        let config = test_config("kill-session");
        backend.create_session(&config).await.unwrap();
        backend.kill_session("kill-session").await.unwrap();
        let sessions = backend.list_sessions().await.unwrap();
        assert!(!sessions.contains(&"kill-session".to_string()));
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn send_command_to_valid_session() {
        let backend = PtyBackend::new("test");
        let config = test_config("cmd-session");
        backend.create_session(&config).await.unwrap();
        // echo command — just checks it doesn't error
        let result = backend.send_command("cmd-session", "echo hello").await;
        assert!(result.is_ok());
        backend.kill_session("cmd-session").await.unwrap();
    }

    #[tokio::test]
    async fn send_command_to_missing_session_errors() {
        let backend = PtyBackend::new("test");
        let result = backend.send_command("no-such-session", "echo hi").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn session_health_healthy_after_create() {
        let backend = PtyBackend::new("test");
        let config = test_config("health-session");
        backend.create_session(&config).await.unwrap();
        let health = backend.session_health("health-session").await.unwrap();
        assert_eq!(health, SessionHealth::Healthy);
        backend.kill_session("health-session").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn shutdown_all_sessions_clears_map() {
        let backend = PtyBackend::new("test");
        for i in 0..3 {
            let config = test_config(&format!("shutdown-session-{i}"));
            backend.create_session(&config).await.unwrap();
        }
        backend.shutdown_all_sessions().await.unwrap();
        let sessions = backend.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn resize_session_noop_for_missing() {
        // Resizing a session that doesn't exist should return Ok(()) silently.
        let backend = PtyBackend::new("test");
        let result = backend.resize_session("nonexistent", 120, 40).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn session_output_stream_none_for_missing() {
        let backend = PtyBackend::new("test");
        let stream = backend.session_output_stream("nonexistent").await.unwrap();
        assert!(stream.is_none());
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn resize_session_ok_for_live_session() {
        let backend = PtyBackend::new("test");
        let config = test_config("resize-session");
        backend.create_session(&config).await.unwrap();
        let result = backend.resize_session("resize-session", 120, 40).await;
        assert!(result.is_ok(), "resize should succeed: {result:?}");
        backend.kill_session("resize-session").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires PTY support (not available in sandbox/CI)"]
    async fn session_output_stream_some_for_live_session() {
        let backend = PtyBackend::new("test");
        let config = test_config("stream-session");
        backend.create_session(&config).await.unwrap();
        let stream = backend.session_output_stream("stream-session").await.unwrap();
        assert!(stream.is_some(), "should return PtyOutputStream for live PTY session");
        backend.kill_session("stream-session").await.unwrap();
    }
}
