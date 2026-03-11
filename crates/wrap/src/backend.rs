//! Execution backend abstraction for agent session management.
//!
//! This module defines the [`ExecutionBackend`] trait, which provides a
//! uniform async interface for launching and managing agent sessions across
//! different execution environments (tmux, Docker, Podman, etc.).
//!
//! # Implementations
//!
//! - [`TmuxBackend`] — wraps [`TmuxManager`](crate::tmux::TmuxManager) with
//!   async compatibility via `spawn_blocking`.
//!
//! # Examples
//!
//! ```no_run
//! use wrap::backend::{ExecutionBackend, TmuxBackend, SessionConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let backend = TmuxBackend::new("agentd");
//!
//! let config = SessionConfig {
//!     session_name: "my-session".into(),
//!     working_dir: "/home/user/project".into(),
//!     agent_type: "claude-code".into(),
//!     model_provider: "anthropic".into(),
//!     model_name: "claude-sonnet-4.5".into(),
//!     layout: None,
//!     network_policy: None,
//! };
//!
//! backend.create_session(&config).await?;
//! backend.launch_agent(&config).await?;
//!
//! let sessions = backend.list_sessions().await?;
//! println!("Active sessions: {:?}", sessions);
//! # Ok(())
//! # }
//! ```

use crate::tmux::TmuxManager;
use crate::types::TmuxLayout;
use async_trait::async_trait;

/// Configuration for creating and launching an agent session.
///
/// This is a backend-agnostic configuration struct that avoids coupling
/// the trait to any specific service's request types.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Name for the session
    pub session_name: String,

    /// Working directory for the agent
    pub working_dir: String,

    /// Agent type (e.g., "claude-code", "opencode", "gemini")
    pub agent_type: String,

    /// Model provider (e.g., "anthropic", "openai", "ollama")
    pub model_provider: String,

    /// Model name (e.g., "claude-sonnet-4.5", "gpt-4")
    pub model_name: String,

    /// Optional layout configuration (tmux-specific, ignored by other backends)
    pub layout: Option<TmuxLayout>,

    /// Optional network policy override for Docker backends.
    ///
    /// When `None`, the backend's default policy is used. Tmux backends
    /// ignore this field.
    pub network_policy: Option<crate::docker::NetworkPolicy>,
}

/// Async trait for execution backends that manage agent sessions.
///
/// Each implementation wraps a specific execution environment (tmux, Docker,
/// Podman, etc.) and exposes a uniform async interface for the orchestrator
/// and wrap service to consume.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `dyn ExecutionBackend`.
#[async_trait]
pub trait ExecutionBackend: Send + Sync {
    /// Creates a new session in the backend environment.
    ///
    /// For tmux this creates a detached session; for containers this would
    /// create and start a container.
    async fn create_session(&self, config: &SessionConfig) -> anyhow::Result<()>;

    /// Launches an agent CLI inside an existing session.
    ///
    /// The agent command is determined by `config.agent_type` and model settings.
    async fn launch_agent(&self, config: &SessionConfig) -> anyhow::Result<()>;

    /// Checks whether a session with the given name exists and is active.
    async fn session_exists(&self, session_name: &str) -> anyhow::Result<bool>;

    /// Terminates a session and all processes within it.
    ///
    /// Implementations should be idempotent — killing a non-existent session
    /// should not return an error.
    async fn kill_session(&self, session_name: &str) -> anyhow::Result<()>;

    /// Sends an arbitrary command to an existing session.
    ///
    /// This is used by the orchestrator to launch a fully constructed CLI
    /// command (with `--sdk-url`, env vars, model flags, etc.) inside a
    /// session that was previously created via [`create_session`].
    async fn send_command(&self, session_name: &str, command: &str) -> anyhow::Result<()>;

    /// Lists all active session names managed by this backend.
    async fn list_sessions(&self) -> anyhow::Result<Vec<String>>;

    /// Returns the session name prefix used by this backend.
    fn prefix(&self) -> &str;

    /// Returns the WebSocket URL for streaming agent output, if supported.
    ///
    /// Not all backends support WebSocket streaming. Returns `None` by default.
    fn agent_ws_url(&self, _session_name: &str) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------------
// TmuxBackend
// ---------------------------------------------------------------------------

/// Async execution backend backed by tmux sessions.
///
/// Wraps [`TmuxManager`] and adapts its synchronous `std::process::Command`
/// calls to async using [`tokio::task::spawn_blocking`].
///
/// # Examples
///
/// ```
/// use wrap::backend::TmuxBackend;
///
/// let backend = TmuxBackend::new("agentd");
/// assert_eq!(backend.prefix(), "agentd");
/// ```
#[derive(Debug, Clone)]
pub struct TmuxBackend {
    tmux: TmuxManager,
}

impl TmuxBackend {
    /// Creates a new `TmuxBackend` with the given session name prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self { tmux: TmuxManager::new(prefix) }
    }

    /// Returns a reference to the inner [`TmuxManager`].
    pub fn inner(&self) -> &TmuxManager {
        &self.tmux
    }
}

/// Build the shell command string for launching an agent CLI.
fn build_agent_command(config: &SessionConfig) -> anyhow::Result<String> {
    match config.agent_type.as_str() {
        "claude-code" => Ok("claude".to_string()),
        "crush" => Ok("crush".to_string()),
        "opencode" => Ok(format!(
            "opencode --model-provider {} --model {}",
            config.model_provider, config.model_name
        )),
        "gemini" => Ok(format!("gemini --model {}", config.model_name)),
        "general" => Ok(std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())),
        other => Err(anyhow::anyhow!("Unsupported agent type: {}", other)),
    }
}

#[async_trait]
impl ExecutionBackend for TmuxBackend {
    async fn create_session(&self, config: &SessionConfig) -> anyhow::Result<()> {
        let tmux = self.tmux.clone();
        let session_name = config.session_name.clone();
        let working_dir = config.working_dir.clone();
        let layout = config.layout.clone();

        tokio::task::spawn_blocking(move || {
            tmux.create_session(&session_name, &working_dir, layout.as_ref())
        })
        .await?
    }

    async fn launch_agent(&self, config: &SessionConfig) -> anyhow::Result<()> {
        let command = build_agent_command(config)?;
        let tmux = self.tmux.clone();
        let session_name = config.session_name.clone();

        tokio::task::spawn_blocking(move || tmux.send_command(&session_name, &command)).await?
    }

    async fn session_exists(&self, session_name: &str) -> anyhow::Result<bool> {
        let tmux = self.tmux.clone();
        let name = session_name.to_string();

        tokio::task::spawn_blocking(move || tmux.session_exists(&name)).await?
    }

    async fn send_command(&self, session_name: &str, command: &str) -> anyhow::Result<()> {
        let tmux = self.tmux.clone();
        let name = session_name.to_string();
        let cmd = command.to_string();

        tokio::task::spawn_blocking(move || tmux.send_command(&name, &cmd)).await?
    }

    async fn kill_session(&self, session_name: &str) -> anyhow::Result<()> {
        let tmux = self.tmux.clone();
        let name = session_name.to_string();

        tokio::task::spawn_blocking(move || tmux.kill_session(&name)).await?
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        let tmux = self.tmux.clone();

        tokio::task::spawn_blocking(move || tmux.list_sessions()).await?
    }

    fn prefix(&self) -> &str {
        self.tmux.prefix()
    }

    fn agent_ws_url(&self, _session_name: &str) -> Option<String> {
        // Tmux sessions don't natively support WebSocket streaming
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_backend_new_sets_prefix() {
        let backend = TmuxBackend::new("test-prefix");
        assert_eq!(backend.prefix(), "test-prefix");
    }

    #[test]
    fn tmux_backend_inner_returns_manager() {
        let backend = TmuxBackend::new("agentd");
        assert_eq!(backend.inner().prefix(), "agentd");
    }

    #[test]
    fn tmux_backend_clone() {
        let backend = TmuxBackend::new("agentd");
        let cloned = backend.clone();
        assert_eq!(cloned.prefix(), "agentd");
    }

    #[test]
    fn tmux_backend_ws_url_returns_none() {
        let backend = TmuxBackend::new("agentd");
        assert_eq!(backend.agent_ws_url("some-session"), None);
    }

    #[test]
    fn build_agent_command_claude_code() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: "claude-sonnet-4.5".into(),
            layout: None,
            network_policy: None,
        };
        assert_eq!(build_agent_command(&config).unwrap(), "claude");
    }

    #[test]
    fn build_agent_command_opencode() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "opencode".into(),
            model_provider: "openai".into(),
            model_name: "gpt-4".into(),
            layout: None,
            network_policy: None,
        };
        assert_eq!(
            build_agent_command(&config).unwrap(),
            "opencode --model-provider openai --model gpt-4"
        );
    }

    #[test]
    fn build_agent_command_gemini() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "gemini".into(),
            model_provider: "google".into(),
            model_name: "gemini-pro".into(),
            layout: None,
            network_policy: None,
        };
        assert_eq!(build_agent_command(&config).unwrap(), "gemini --model gemini-pro");
    }

    #[test]
    fn build_agent_command_crush() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "crush".into(),
            model_provider: "anthropic".into(),
            model_name: "claude-sonnet-4.5".into(),
            layout: None,
            network_policy: None,
        };
        assert_eq!(build_agent_command(&config).unwrap(), "crush");
    }

    #[test]
    fn build_agent_command_unsupported() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "unknown-agent".into(),
            model_provider: "none".into(),
            model_name: "none".into(),
            layout: None,
            network_policy: None,
        };
        let err = build_agent_command(&config).unwrap_err();
        assert!(err.to_string().contains("Unsupported agent type"));
    }

    #[test]
    fn session_config_debug() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: "claude-sonnet-4.5".into(),
            layout: None,
            network_policy: None,
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("test"));
        assert!(debug.contains("claude-code"));
    }

    #[test]
    fn session_config_clone() {
        let config = SessionConfig {
            session_name: "test".into(),
            working_dir: "/tmp".into(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: "claude-sonnet-4.5".into(),
            layout: Some(TmuxLayout { layout_type: "vertical".into(), panes: Some(2) }),
            network_policy: None,
        };
        let cloned = config.clone();
        assert_eq!(cloned.session_name, "test");
        assert!(cloned.layout.is_some());
    }

    /// Verify that `TmuxBackend` is object-safe (can be used as `dyn ExecutionBackend`).
    #[test]
    fn trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn ExecutionBackend) {}
        let backend = TmuxBackend::new("agentd");
        _assert_object_safe(&backend);
    }

    /// Verify Send + Sync bounds are satisfied.
    #[test]
    fn tmux_backend_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<TmuxBackend>();
    }
}
