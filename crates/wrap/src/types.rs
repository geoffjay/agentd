//! Type definitions for the wrap service.
//!
//! This module defines the request and response types used for communicating
//! with the wrap service REST API.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Backend selection
// ---------------------------------------------------------------------------

/// The execution backend used to run agent sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    /// tmux-based sessions (default)
    #[default]
    Tmux,
    /// Docker container sessions
    Docker,
    /// In-process PTY sessions
    Pty,
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendType::Tmux => write!(f, "tmux"),
            BackendType::Docker => write!(f, "docker"),
            BackendType::Pty => write!(f, "pty"),
        }
    }
}

impl BackendType {
    /// Read the backend type from the `AGENTD_BACKEND` environment variable.
    ///
    /// Returns [`BackendType::Tmux`] if the variable is unset or unrecognised.
    pub fn from_env() -> Self {
        match std::env::var("AGENTD_BACKEND").as_deref() {
            Ok("docker") => BackendType::Docker,
            Ok("pty") => BackendType::Pty,
            _ => BackendType::Tmux,
        }
    }

    /// Read the backend type from `AGENTD_BACKEND`, returning an error for
    /// unrecognised values.
    ///
    /// Unlike [`from_env`](Self::from_env), this method rejects unknown values
    /// instead of silently falling back to `Tmux`. Use this in service entry
    /// points where an invalid configuration should abort startup.
    ///
    /// Returns [`BackendType::Tmux`] when `AGENTD_BACKEND` is unset.
    ///
    /// # Errors
    ///
    /// Returns an error if `AGENTD_BACKEND` is set to an unrecognised value.
    pub fn from_env_strict() -> anyhow::Result<Self> {
        match std::env::var("AGENTD_BACKEND").as_deref() {
            Ok("tmux") | Err(_) => Ok(BackendType::Tmux),
            Ok("docker") => Ok(BackendType::Docker),
            Ok("pty") => Ok(BackendType::Pty),
            Ok(other) => anyhow::bail!(
                "Unknown AGENTD_BACKEND value '{}'. Valid options: tmux, docker, pty",
                other
            ),
        }
    }

    /// Returns the capabilities exposed by this backend.
    pub fn capabilities(&self) -> Vec<String> {
        match self {
            BackendType::Tmux => vec!["attach-tmux".to_string()],
            BackendType::Docker => vec!["health-check".to_string(), "logs".to_string()],
            BackendType::Pty => vec!["terminal".to_string(), "interactive".to_string()],
        }
    }
}

/// Information about the active execution backend returned by `GET /info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendInfo {
    /// The active backend type
    pub backend_type: BackendType,
    /// Service version
    pub version: String,
    /// Capabilities supported by this backend
    pub capabilities: Vec<String>,
}

/// Request to launch an agent in a session.
///
/// Contains all configuration needed to start an agent CLI with proper
/// environment and parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    /// Project name (used for session naming)
    pub project_name: String,

    /// Working directory path for the agent
    pub project_path: String,

    /// Agent type (e.g., "claude-code", "opencode", "gemini")
    pub agent_type: String,

    /// Model provider (e.g., "anthropic", "openai", "ollama")
    pub model_provider: String,

    /// Model name (e.g., "claude-sonnet-4.5", "gpt-4")
    pub model_name: String,

    /// Optional tmux layout configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<TmuxLayout>,

    /// Requested backend override (informational; the service uses whatever
    /// backend is configured at startup via `AGENTD_BACKEND`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
}

/// Response from launching an agent.
///
/// Contains information about the created session and initial health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResponse {
    /// Whether the agent started successfully
    pub success: bool,

    /// Name of the tmux session (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,

    /// Human-readable message
    pub message: String,

    /// Optional error message if launch failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Re-export shared HealthResponse from agentd-common.
pub use agentd_common::types::HealthResponse;

/// Information about a single agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session name
    pub name: String,

    /// Whether the session is currently active
    pub active: bool,

    /// Execution backend for this session
    #[serde(default)]
    pub backend: BackendType,

    /// Capabilities of this session's backend
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Response listing all active sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListResponse {
    /// List of active sessions
    pub sessions: Vec<SessionInfo>,

    /// Total number of sessions
    pub count: usize,
}

/// Response after killing a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSessionResponse {
    /// Whether the kill was successful
    pub success: bool,

    /// Human-readable message
    pub message: String,
}

/// Tmux layout configuration.
///
/// Defines how the tmux session should be laid out (single pane, split, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxLayout {
    /// Layout type
    ///
    /// Supported values: `single`, `horizontal`, `vertical`, `tiled`
    #[serde(rename = "type")]
    pub layout_type: String,

    /// Number of panes (for split layouts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panes: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_request_serialization() {
        let request = LaunchRequest {
            project_name: "test-project".to_string(),
            project_path: "/tmp/project".to_string(),
            agent_type: "claude-code".to_string(),
            model_provider: "anthropic".to_string(),
            model_name: "claude-sonnet-4.5".to_string(),
            layout: None,
            backend: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test-project"));
        assert!(json.contains("claude-code"));
    }

    #[test]
    fn test_launch_request_with_layout() {
        let request = LaunchRequest {
            project_name: "test-project".to_string(),
            project_path: "/tmp/project".to_string(),
            agent_type: "opencode".to_string(),
            model_provider: "openai".to_string(),
            model_name: "gpt-4".to_string(),
            layout: Some(TmuxLayout { layout_type: "vertical".to_string(), panes: Some(2) }),
            backend: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("vertical"));
        assert!(json.contains("opencode"));
    }

    #[test]
    fn test_launch_response_deserialization() {
        let json = r#"{
            "success": true,
            "session_name": "test-session",
            "message": "Success"
        }"#;

        let response: LaunchResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert_eq!(response.session_name, Some("test-session".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_launch_response_with_error() {
        let json = r#"{
            "success": false,
            "message": "Failed to start agent",
            "error": "Failed to start agent"
        }"#;

        let response: LaunchResponse = serde_json::from_str(json).unwrap();
        assert!(!response.success);
        assert_eq!(response.error, Some("Failed to start agent".to_string()));
    }

    #[test]
    fn test_health_response_deserialization() {
        let response = HealthResponse::ok("agentd-wrap", "0.1.0");
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "ok");
        assert_eq!(deserialized.service, "agentd-wrap");
        assert_eq!(deserialized.version, "0.1.0");
    }

    #[test]
    fn test_tmux_layout_serialization() {
        let layout = TmuxLayout { layout_type: "horizontal".to_string(), panes: Some(3) };

        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.contains("horizontal"));
        assert!(json.contains("3"));
    }

    fn make_session(name: &str) -> SessionInfo {
        SessionInfo {
            name: name.to_string(),
            active: true,
            backend: BackendType::default(),
            capabilities: vec![],
        }
    }

    #[test]
    fn test_session_info_serialization() {
        let session = make_session("my-session");

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("my-session"));
        assert!(json.contains("true"));

        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "my-session");
        assert!(deserialized.active);
    }

    #[test]
    fn test_session_list_response_serialization() {
        let response = SessionListResponse {
            sessions: vec![make_session("session-1"), make_session("session-2")],
            count: 2,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("session-1"));
        assert!(json.contains("session-2"));

        let deserialized: SessionListResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.count, 2);
        assert_eq!(deserialized.sessions.len(), 2);
    }

    #[test]
    fn test_session_list_response_empty() {
        let response = SessionListResponse { sessions: vec![], count: 0 };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: SessionListResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.count, 0);
        assert!(deserialized.sessions.is_empty());
    }

    #[test]
    fn test_kill_session_response_serialization() {
        let response =
            KillSessionResponse { success: true, message: "Session terminated".to_string() };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("Session terminated"));

        let deserialized: KillSessionResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
    }

    // -----------------------------------------------------------------------
    // BackendType::from_env_strict
    // -----------------------------------------------------------------------

    #[test]
    fn from_env_strict_unset_returns_tmux() {
        // Remove any inherited env var so the test is deterministic.
        std::env::remove_var("AGENTD_BACKEND");
        let bt = BackendType::from_env_strict().unwrap();
        assert_eq!(bt, BackendType::Tmux);
    }

    #[test]
    fn from_env_strict_tmux_explicit() {
        std::env::set_var("AGENTD_BACKEND", "tmux");
        let bt = BackendType::from_env_strict().unwrap();
        assert_eq!(bt, BackendType::Tmux);
        std::env::remove_var("AGENTD_BACKEND");
    }

    #[test]
    fn from_env_strict_docker() {
        std::env::set_var("AGENTD_BACKEND", "docker");
        let bt = BackendType::from_env_strict().unwrap();
        assert_eq!(bt, BackendType::Docker);
        std::env::remove_var("AGENTD_BACKEND");
    }

    #[test]
    fn from_env_strict_pty() {
        std::env::set_var("AGENTD_BACKEND", "pty");
        let bt = BackendType::from_env_strict().unwrap();
        assert_eq!(bt, BackendType::Pty);
        std::env::remove_var("AGENTD_BACKEND");
    }

    #[test]
    fn from_env_strict_unknown_errors() {
        std::env::set_var("AGENTD_BACKEND", "kubernetes");
        let result = BackendType::from_env_strict();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("kubernetes"), "error should name the bad value: {msg}");
        assert!(msg.contains("tmux"), "error should list valid options: {msg}");
        std::env::remove_var("AGENTD_BACKEND");
    }

    #[test]
    fn pty_has_no_health_check_capability() {
        assert!(
            !BackendType::Pty.capabilities().contains(&"health-check".to_string()),
            "PTY backend must not advertise health-check capability"
        );
    }

    #[test]
    fn pty_has_terminal_and_interactive_capabilities() {
        let caps = BackendType::Pty.capabilities();
        assert!(caps.contains(&"terminal".to_string()));
        assert!(caps.contains(&"interactive".to_string()));
    }

    #[test]
    fn docker_has_health_check_capability() {
        assert!(BackendType::Docker.capabilities().contains(&"health-check".to_string()));
    }
}
