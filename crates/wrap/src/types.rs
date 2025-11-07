//! Type definitions for the wrap service.
//!
//! This module defines the request and response types used for communicating
//! with the wrap service REST API.

use serde::{Deserialize, Serialize};

/// Request to launch an agent in a tmux session.
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

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,

    /// Service version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
        let json = r#"{
            "status": "ok",
            "version": "0.1.0"
        }"#;

        let response: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "ok");
        assert_eq!(response.version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_tmux_layout_serialization() {
        let layout = TmuxLayout { layout_type: "horizontal".to_string(), panes: Some(3) };

        let json = serde_json::to_string(&layout).unwrap();
        assert!(json.contains("horizontal"));
        assert!(json.contains("3"));
    }
}
