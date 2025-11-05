//! Request and response types for the ask service.
//!
//! This module defines all data structures used in API requests and responses,
//! as well as types shared with the notification service. All types implement
//! `Serialize` and `Deserialize` for JSON communication.
//!
//! # Type Categories
//!
//! - **Notification Types**: Aligned with the notification service schema
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

/// Source of a notification.
///
/// Identifies where a notification originated from. Currently only supports
/// the ask service, but the enum structure allows for future expansion.
///
/// This type must match the notification service's schema exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationSource {
    #[serde(rename = "AskService")]
    AskService { request_id: Uuid },
}

/// Lifetime behavior of a notification.
///
/// Determines whether a notification persists indefinitely or expires after a time.
///
/// # Variants
///
/// - `Ephemeral` - Notification expires at a specific timestamp
/// - `Persistent` - Notification remains until explicitly dismissed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationLifetime {
    /// Ephemeral notification that expires at the given timestamp.
    #[serde(rename = "Ephemeral")]
    Ephemeral { expires_at: DateTime<Utc> },
    /// Persistent notification that remains until dismissed.
    #[serde(rename = "Persistent")]
    Persistent,
}

impl NotificationLifetime {
    /// Creates an ephemeral notification that expires after the given duration.
    ///
    /// # Arguments
    ///
    /// - `duration` - How long from now until the notification expires
    ///
    /// # Returns
    ///
    /// Returns a [`NotificationLifetime::Ephemeral`] variant with calculated expiry time.
    ///
    /// # Examples
    ///
    /// ```
    /// use ask::types::NotificationLifetime;
    /// use chrono::Duration;
    ///
    /// // Create a notification that expires in 5 minutes
    /// let lifetime = NotificationLifetime::ephemeral(Duration::minutes(5));
    /// ```
    pub fn ephemeral(duration: chrono::Duration) -> Self {
        NotificationLifetime::Ephemeral { expires_at: Utc::now() + duration }
    }
}

/// Priority level for notifications.
///
/// Indicates the urgency of a notification. Higher priorities may be displayed
/// more prominently or trigger more immediate alerts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NotificationPriority {
    /// Low priority - informational only
    Low,
    /// Normal priority - standard notifications
    Normal,
    /// High priority - important but not urgent
    High,
    /// Urgent - requires immediate attention
    Urgent,
}

/// Status of a notification.
///
/// Tracks the lifecycle state of a notification from creation through completion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NotificationStatus {
    /// Notification created but not yet viewed
    Pending,
    /// User has viewed the notification
    Viewed,
    /// User has responded to the notification
    Responded,
    /// User dismissed the notification
    Dismissed,
    /// Notification expired (for ephemeral notifications)
    Expired,
}

/// Request to create a new notification.
///
/// Contains all information needed to create a notification in the notification service.
///
/// # JSON Example
///
/// ```json
/// {
///   "source": {"AskService": {"request_id": "550e8400-e29b-41d4-a716-446655440000"}},
///   "lifetime": "Persistent",
///   "priority": "Normal",
///   "title": "Question",
///   "message": "Do you want to continue?",
///   "requires_response": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationRequest {
    /// Source identifier for the notification
    pub source: NotificationSource,
    /// Lifetime behavior (ephemeral or persistent)
    pub lifetime: NotificationLifetime,
    /// Priority level
    pub priority: NotificationPriority,
    /// Notification title
    pub title: String,
    /// Notification message body
    pub message: String,
    /// Whether this notification requires a response
    pub requires_response: bool,
}

/// A notification from the notification service.
///
/// Contains all notification data including metadata, status, and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub source: NotificationSource,
    pub lifetime: NotificationLifetime,
    pub priority: NotificationPriority,
    pub status: NotificationStatus,
    pub title: String,
    pub message: String,
    pub requires_response: bool,
    pub response: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to update an existing notification.
///
/// Only includes fields that should be updated. Fields set to `None` are not
/// included in the JSON (via `skip_serializing_if`).
///
/// # JSON Example
///
/// ```json
/// {
///   "status": "Responded",
///   "response": "yes"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotificationRequest {
    /// Updated status (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NotificationStatus>,
    /// User's response text (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

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

/// Response from the `/health` endpoint.
///
/// Provides service status and configuration information.
///
/// # JSON Example
///
/// ```json
/// {
///   "status": "ok",
///   "service": "agentd-ask",
///   "version": "0.1.0",
///   "notification_service_url": "http://localhost:3000"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status ("ok" if healthy)
    pub status: String,
    /// Service name identifier
    pub service: String,
    /// Service version number
    pub version: String,
    /// URL of the notification service
    pub notification_service_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_source_serialization() {
        let source = NotificationSource::AskService { request_id: Uuid::new_v4() };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("AskService"));
        assert!(json.contains("request_id"));

        let deserialized: NotificationSource = serde_json::from_str(&json).unwrap();
        match deserialized {
            NotificationSource::AskService { .. } => {}
        }
    }

    #[test]
    fn test_notification_lifetime_ephemeral() {
        let lifetime = NotificationLifetime::ephemeral(chrono::Duration::minutes(5));
        match lifetime {
            NotificationLifetime::Ephemeral { expires_at } => {
                let now = Utc::now();
                let duration = expires_at - now;
                assert!(duration.num_minutes() >= 4 && duration.num_minutes() <= 5);
            }
            _ => panic!("Expected ephemeral lifetime"),
        }
    }

    #[test]
    fn test_notification_lifetime_serialization() {
        let ephemeral = NotificationLifetime::ephemeral(chrono::Duration::minutes(5));
        let json = serde_json::to_string(&ephemeral).unwrap();
        assert!(json.contains("Ephemeral"));
        assert!(json.contains("expires_at"));

        let persistent = NotificationLifetime::Persistent;
        let json = serde_json::to_string(&persistent).unwrap();
        assert!(json.contains("Persistent"));
    }

    #[test]
    fn test_notification_priority_serialization() {
        let priorities = vec![
            NotificationPriority::Low,
            NotificationPriority::Normal,
            NotificationPriority::High,
            NotificationPriority::Urgent,
        ];

        for priority in priorities {
            let json = serde_json::to_string(&priority).unwrap();
            let deserialized: NotificationPriority = serde_json::from_str(&json).unwrap();
            assert_eq!(std::mem::discriminant(&priority), std::mem::discriminant(&deserialized));
        }
    }

    #[test]
    fn test_notification_status_equality() {
        assert_eq!(NotificationStatus::Pending, NotificationStatus::Pending);
        assert_eq!(NotificationStatus::Viewed, NotificationStatus::Viewed);
        assert_ne!(NotificationStatus::Pending, NotificationStatus::Viewed);
    }

    #[test]
    fn test_notification_status_serialization() {
        let statuses = vec![
            NotificationStatus::Pending,
            NotificationStatus::Viewed,
            NotificationStatus::Responded,
            NotificationStatus::Dismissed,
            NotificationStatus::Expired,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: NotificationStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_create_notification_request_serialization() {
        let request = CreateNotificationRequest {
            source: NotificationSource::AskService { request_id: Uuid::new_v4() },
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: true,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: CreateNotificationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.title, deserialized.title);
        assert_eq!(request.message, deserialized.message);
        assert_eq!(request.requires_response, deserialized.requires_response);
    }

    #[test]
    fn test_update_notification_request_serialization() {
        let request = UpdateNotificationRequest {
            status: Some(NotificationStatus::Responded),
            response: Some("yes".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("status"));
        assert!(json.contains("response"));

        let deserialized: UpdateNotificationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.status, deserialized.status);
        assert_eq!(request.response, deserialized.response);
    }

    #[test]
    fn test_update_notification_request_skip_none() {
        let request = UpdateNotificationRequest { status: None, response: Some("yes".to_string()) };

        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("status"));
        assert!(json.contains("response"));
    }

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
        let response = HealthResponse {
            status: "ok".to_string(),
            service: "agentd-ask".to_string(),
            version: "0.1.0".to_string(),
            notification_service_url: "http://localhost:3000".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: HealthResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response.status, deserialized.status);
        assert_eq!(response.service, deserialized.service);
    }
}
