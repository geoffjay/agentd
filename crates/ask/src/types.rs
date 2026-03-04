//! Request and response types for the ask service.
//!
//! This module defines all data structures used in API requests and responses.
//! Notification types are re-exported from the [`notify`] crate.
//!
//! # Type Categories
//!
//! - **Notification Types**: Re-exported from `notify::types`
//! - **Question Types**: Track questions asked to users
//! - **Check Types**: Different kinds of environment checks
//! - **Request/Response Types**: API endpoint request and response structures
//!
//! # Examples
//!
//! ## Creating a notification request
//!
//! ```
//! use ask::types::{
//!     CreateNotificationRequest, NotificationSource, NotificationLifetime,
//!     NotificationPriority,
//! };
//! use uuid::Uuid;
//!
//! let request = CreateNotificationRequest {
//!     source: NotificationSource::AskService { request_id: Uuid::new_v4() },
//!     lifetime: NotificationLifetime::Persistent,
//!     priority: NotificationPriority::Normal,
//!     title: "Question".to_string(),
//!     message: "Do you want to continue?".to_string(),
//!     requires_response: true,
//! };
//! ```
//!
//! ## Working with question info
//!
//! ```
//! use ask::types::{QuestionInfo, CheckType, QuestionStatus};
//! use chrono::Utc;
//! use uuid::Uuid;
//!
//! let question = QuestionInfo {
//!     question_id: Uuid::new_v4(),
//!     notification_id: Uuid::new_v4(),
//!     check_type: CheckType::TmuxSessions,
//!     asked_at: Utc::now(),
//!     status: QuestionStatus::Pending,
//!     answer: None,
//! };
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export notification types from the notify crate
pub use notify::types::{
    CreateNotificationRequest, Notification, NotificationLifetime, NotificationPriority,
    NotificationSource, NotificationStatus, UpdateNotificationRequest,
};

/// Information about a question asked to the user.
///
/// Tracks the complete state of a question from creation through answer or expiration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionInfo {
    /// Unique ID for this question
    pub question_id: Uuid,
    /// The notification ID from the notification service
    pub notification_id: Uuid,
    /// Type of check that triggered this question
    pub check_type: CheckType,
    /// When the question was asked
    pub asked_at: DateTime<Utc>,
    /// Current status
    pub status: QuestionStatus,
    /// User's answer (if provided)
    pub answer: Option<String>,
}

/// Type of environment check performed.
///
/// Each check type has its own cooldown tracking and may trigger different
/// question templates.
///
/// # Note
///
/// This type implements `Hash` and `Eq` so it can be used as a HashMap key.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CheckType {
    /// Check for running tmux sessions
    TmuxSessions,
}

impl CheckType {
    /// Returns the string representation of the check type.
    ///
    /// Used for logging and API responses.
    ///
    /// # Examples
    ///
    /// ```
    /// use ask::types::CheckType;
    ///
    /// assert_eq!(CheckType::TmuxSessions.as_str(), "tmux_sessions");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckType::TmuxSessions => "tmux_sessions",
        }
    }
}

/// Status of a question.
///
/// Tracks whether a question is awaiting response, has been answered, or expired.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum QuestionStatus {
    /// Question is awaiting user response
    Pending,
    /// User has provided an answer
    Answered,
    /// Question expired before being answered
    Expired,
}

/// Result of a tmux session check.
///
/// Contains information about whether tmux sessions are running and details
/// about active sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxCheckResult {
    pub running: bool,
    pub session_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<String>>,
}

/// Response from the `/trigger` endpoint.
///
/// Contains information about what checks were run, which notifications were sent,
/// and the detailed results of each check.
///
/// # JSON Example
///
/// ```json
/// {
///   "checks_run": ["tmux_sessions"],
///   "notifications_sent": ["550e8400-e29b-41d4-a716-446655440000"],
///   "results": {
///     "tmux_sessions": {
///       "running": false,
///       "session_count": 0,
///       "sessions": []
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerResponse {
    /// List of check types that were executed
    pub checks_run: Vec<String>,
    /// List of notification IDs that were created
    pub notifications_sent: Vec<Uuid>,
    /// Detailed results for each check
    pub results: TriggerResults,
}

/// Detailed results from each check type.
///
/// Currently only contains tmux session check results, but structured to allow
/// adding more check types in the future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerResults {
    /// Result of the tmux session check
    pub tmux_sessions: TmuxCheckResult,
}

/// Request to submit an answer to a question.
///
/// Used by the `/answer` endpoint.
///
/// # JSON Example
///
/// ```json
/// {
///   "question_id": "550e8400-e29b-41d4-a716-446655440000",
///   "answer": "yes"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerRequest {
    /// The UUID of the question being answered
    pub question_id: Uuid,
    /// The user's answer as free-form text
    pub answer: String,
}

/// Response from the `/answer` endpoint.
///
/// Confirms whether the answer was recorded successfully.
///
/// # JSON Example
///
/// ```json
/// {
///   "success": true,
///   "message": "Answer recorded for question 550e8400-e29b-41d4-a716-446655440000",
///   "question_id": "550e8400-e29b-41d4-a716-446655440000"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerResponse {
    /// Whether the answer was successfully recorded
    pub success: bool,
    /// Human-readable confirmation message
    pub message: String,
    /// The question ID that was answered
    pub question_id: Uuid,
}

// Re-export shared HealthResponse from agentd-common.
pub use agentd_common::types::HealthResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_type_as_str() {
        assert_eq!(CheckType::TmuxSessions.as_str(), "tmux_sessions");
    }

    #[test]
    fn test_check_type_equality() {
        assert_eq!(CheckType::TmuxSessions, CheckType::TmuxSessions);
    }

    #[test]
    fn test_check_type_serialization() {
        let check_type = CheckType::TmuxSessions;
        let json = serde_json::to_string(&check_type).unwrap();
        let deserialized: CheckType = serde_json::from_str(&json).unwrap();
        assert_eq!(check_type, deserialized);
    }

    #[test]
    fn test_question_status_equality() {
        assert_eq!(QuestionStatus::Pending, QuestionStatus::Pending);
        assert_ne!(QuestionStatus::Pending, QuestionStatus::Answered);
    }

    #[test]
    fn test_question_info_serialization() {
        let question = QuestionInfo {
            question_id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            check_type: CheckType::TmuxSessions,
            asked_at: Utc::now(),
            status: QuestionStatus::Pending,
            answer: None,
        };

        let json = serde_json::to_string(&question).unwrap();
        let deserialized: QuestionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(question.question_id, deserialized.question_id);
        assert_eq!(question.status, deserialized.status);
        assert_eq!(question.answer, deserialized.answer);
    }

    #[test]
    fn test_tmux_check_result_serialization() {
        let result = TmuxCheckResult {
            running: true,
            session_count: 3,
            sessions: Some(vec!["main".to_string(), "work".to_string()]),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: TmuxCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.running, deserialized.running);
        assert_eq!(result.session_count, deserialized.session_count);
        assert_eq!(result.sessions, deserialized.sessions);
    }

    #[test]
    fn test_tmux_check_result_skip_none_sessions() {
        let result = TmuxCheckResult { running: false, session_count: 0, sessions: None };

        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("sessions"));
    }

    #[test]
    fn test_trigger_response_serialization() {
        let response = TriggerResponse {
            checks_run: vec!["tmux_sessions".to_string()],
            notifications_sent: vec![Uuid::new_v4()],
            results: TriggerResults {
                tmux_sessions: TmuxCheckResult {
                    running: false,
                    session_count: 0,
                    sessions: Some(vec![]),
                },
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: TriggerResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.checks_run, deserialized.checks_run);
        assert_eq!(response.notifications_sent.len(), deserialized.notifications_sent.len());
    }

    #[test]
    fn test_answer_request_deserialization() {
        let json = r#"{"question_id":"550e8400-e29b-41d4-a716-446655440000","answer":"yes"}"#;
        let request: AnswerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.answer, "yes");
    }

    #[test]
    fn test_answer_response_serialization() {
        let response = AnswerResponse {
            success: true,
            message: "Answer recorded".to_string(),
            question_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("message"));
        assert!(json.contains("question_id"));
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse::ok("agentd-ask", "0.1.0")
            .with_detail("notification_service_url", serde_json::json!("http://localhost:7004"));

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.service, deserialized.service);
    }
}
