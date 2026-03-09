//! Request and response types for the hook service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use agentd_common::types::HealthResponse;

/// The kind of hook event being reported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookKind {
    /// A shell command completed (success or failure)
    Shell,
    /// A git hook fired (pre-commit, post-commit, etc.)
    Git,
    /// A generic system event
    System,
}

/// A hook event received from a shell or git integration.
///
/// Shell hooks send this payload when commands complete; git hooks send it
/// when repository operations occur.
///
/// # JSON Example
///
/// ```json
/// {
///   "kind": "shell",
///   "command": "cargo build",
///   "exit_code": 0,
///   "duration_ms": 1234,
///   "output": "   Compiling …"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    /// Type of hook event
    pub kind: HookKind,
    /// Command or hook name that triggered the event
    pub command: String,
    /// Process exit code (0 = success)
    pub exit_code: i32,
    /// Execution duration in milliseconds
    #[serde(default)]
    pub duration_ms: u64,
    /// Captured output (stdout/stderr), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Arbitrary metadata (e.g., git branch, working directory)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

/// A recorded hook event, as stored and returned by the service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// Unique event identifier
    pub id: Uuid,
    /// When the event was received by the service
    pub received_at: DateTime<Utc>,
    /// The original event payload
    #[serde(flatten)]
    pub event: HookEvent,
}

/// Response from `POST /events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventResponse {
    /// Whether the event was recorded successfully
    pub success: bool,
    /// Unique ID assigned to this event
    pub event_id: Uuid,
    /// Human-readable confirmation
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_kind_serialization() {
        assert_eq!(serde_json::to_string(&HookKind::Shell).unwrap(), r#""shell""#);
        assert_eq!(serde_json::to_string(&HookKind::Git).unwrap(), r#""git""#);
        assert_eq!(serde_json::to_string(&HookKind::System).unwrap(), r#""system""#);
    }

    #[test]
    fn test_hook_event_roundtrip() {
        let event = HookEvent {
            kind: HookKind::Shell,
            command: "cargo test".to_string(),
            exit_code: 0,
            duration_ms: 3000,
            output: Some("test result: ok".to_string()),
            metadata: std::collections::HashMap::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.command, "cargo test");
        assert_eq!(decoded.exit_code, 0);
        assert_eq!(decoded.kind, HookKind::Shell);
    }

    #[test]
    fn test_hook_event_missing_output_omitted() {
        let event = HookEvent {
            kind: HookKind::Git,
            command: "pre-commit".to_string(),
            exit_code: 0,
            duration_ms: 100,
            output: None,
            metadata: std::collections::HashMap::new(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("output"), "output field should be omitted when None");
    }

    #[test]
    fn test_event_response_serialization() {
        let resp = EventResponse {
            success: true,
            event_id: Uuid::new_v4(),
            message: "Event recorded".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("event_id"));
        assert!(json.contains("message"));
    }

    #[test]
    fn test_health_response_creation() {
        let resp = HealthResponse::ok("agentd-hook", "0.2.0");
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.service, "agentd-hook");
    }
}
