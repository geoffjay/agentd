//! Type definitions for the notification system.
//!
//! This module contains all types used for creating, updating, and managing
//! notifications in the agentd system. All types are serializable with serde
//! for JSON API communication.
//!
//! # Notification Lifecycle
//!
//! 1. **Create**: A notification is created with a source, lifetime, priority, and content
//! 2. **View**: The notification status can transition to `Viewed`
//! 3. **Respond**: If response is required, user provides a response (status becomes `Responded`)
//! 4. **Dismiss**: The notification can be dismissed (status becomes `Dismissed`)
//! 5. **Expire**: Ephemeral notifications expire after their timeout (status becomes `Expired`)
//!
//! # JSON Serialization
//!
//! All types use `serde` with `snake_case` field names for JSON compatibility
//! with the REST API. Enums use tagged representation for type-safe deserialization.
//!
//! # Examples
//!
//! ## Creating a notification request
//!
//! ```rust
//! use agentd_cli::types::*;
//! use chrono::{Duration, Utc};
//!
//! let request = CreateNotificationRequest {
//!     source: NotificationSource::System,
//!     lifetime: NotificationLifetime::Persistent,
//!     priority: NotificationPriority::High,
//!     title: "Build Failed".to_string(),
//!     message: "Tests failed on main branch".to_string(),
//!     requires_response: true,
//! };
//!
//! // Serialize to JSON
//! let json = serde_json::to_string(&request).unwrap();
//! ```
//!
//! ## Parsing priority from string
//!
//! ```rust
//! use agentd_cli::types::NotificationPriority;
//!
//! let priority: NotificationPriority = "high".parse().unwrap();
//! assert_eq!(priority, NotificationPriority::High);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Source of a notification.
///
/// Each notification originates from a specific source which determines its context
/// and handling. The source is serialized as a tagged enum with a `type` field.
///
/// # JSON Representation
///
/// ```json
/// // System notification
/// {"type": "system"}
///
/// // Ask service notification
/// {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440000"}
///
/// // Monitor service notification
/// {"type": "monitor_service", "alert_type": "disk_space"}
///
/// // Agent hook notification
/// {"type": "agent_hook", "agent_id": "cli", "hook_type": "pre-commit"}
/// ```
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::NotificationSource;
/// use uuid::Uuid;
///
/// // Create a system notification
/// let source = NotificationSource::System;
///
/// // Create an ask service notification
/// let source = NotificationSource::AskService {
///     request_id: Uuid::new_v4(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationSource {
    /// Notification from an agent hook requiring user input.
    ///
    /// Agent hooks can pause execution and request user input through
    /// the notification system. Common hook types include pre-commit,
    /// pre-push, and custom workflow hooks.
    AgentHook {
        /// ID of the agent that created the hook
        agent_id: String,
        /// Type of hook (e.g., "pre-commit", "pre-push")
        hook_type: String,
    },
    /// Notification from the agentd-ask service.
    ///
    /// The ask service periodically checks conditions and creates notifications
    /// when user attention is required.
    AskService {
        /// UUID of the ask service request
        request_id: Uuid,
    },
    /// Notification from the agentd-monitor service.
    ///
    /// The monitor service watches system metrics and creates alerts when
    /// thresholds are exceeded or anomalies are detected.
    MonitorService {
        /// Type of monitoring alert (e.g., "disk_space", "cpu_usage")
        alert_type: String,
    },
    /// Generic system notification.
    ///
    /// System notifications are created for general purpose alerts that don't
    /// originate from a specific service.
    System,
}

/// Lifetime behavior of a notification.
///
/// Notifications can either expire after a timeout (Ephemeral) or remain
/// until explicitly dismissed (Persistent). The lifetime strategy affects
/// how the system handles cleanup and display.
///
/// # JSON Representation
///
/// ```json
/// // Persistent notification
/// {"type": "persistent"}
///
/// // Ephemeral notification
/// {"type": "ephemeral", "expires_at": "2025-01-01T12:00:00Z"}
/// ```
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::NotificationLifetime;
/// use chrono::{Duration, Utc};
///
/// // Create a persistent notification
/// let lifetime = NotificationLifetime::Persistent;
///
/// // Create an ephemeral notification that expires in 1 hour
/// let lifetime = NotificationLifetime::Ephemeral {
///     expires_at: Utc::now() + Duration::hours(1),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationLifetime {
    /// Ephemeral notifications expire after a timeout.
    ///
    /// Once expired, the notification status transitions to `Expired` and
    /// it may be automatically removed from the active list. Useful for
    /// time-sensitive alerts that become irrelevant.
    Ephemeral {
        /// UTC timestamp when the notification expires
        expires_at: DateTime<Utc>,
    },
    /// Persistent notifications remain until explicitly dismissed.
    ///
    /// These notifications stay in the system indefinitely until the user
    /// takes action. Use for important notifications that require user
    /// acknowledgment.
    Persistent,
}

/// Priority level for notifications.
///
/// Priority determines the urgency and display order of notifications.
/// Higher priority notifications appear first and may trigger different
/// notification behaviors (e.g., sounds, pop-ups).
///
/// Priorities are ordered: Low < Normal < High < Urgent
///
/// # JSON Representation
///
/// Priority values serialize as lowercase strings:
/// ```json
/// "low", "normal", "high", "urgent"
/// ```
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::NotificationPriority;
///
/// // Parse from string (case-insensitive)
/// let priority: NotificationPriority = "high".parse().unwrap();
/// assert_eq!(priority, NotificationPriority::High);
///
/// // Priorities are ordered
/// assert!(NotificationPriority::Low < NotificationPriority::High);
/// assert!(NotificationPriority::High < NotificationPriority::Urgent);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    /// Low priority - informational, non-urgent notifications.
    ///
    /// Examples: routine updates, status reports
    Low,
    /// Normal priority - default for most notifications.
    ///
    /// Examples: standard alerts, completed tasks
    Normal,
    /// High priority - important notifications requiring attention.
    ///
    /// Examples: build failures, test failures
    High,
    /// Urgent priority - critical notifications requiring immediate action.
    ///
    /// Examples: production outages, security alerts
    Urgent,
}

impl std::str::FromStr for NotificationPriority {
    type Err = anyhow::Error;

    /// Parse a priority from a string (case-insensitive).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agentd_cli::types::NotificationPriority;
    ///
    /// assert_eq!("low".parse::<NotificationPriority>().unwrap(), NotificationPriority::Low);
    /// assert_eq!("HIGH".parse::<NotificationPriority>().unwrap(), NotificationPriority::High);
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(NotificationPriority::Low),
            "normal" => Ok(NotificationPriority::Normal),
            "high" => Ok(NotificationPriority::High),
            "urgent" => Ok(NotificationPriority::Urgent),
            _ => Err(anyhow::anyhow!("Invalid priority: {s}")),
        }
    }
}

/// Current status of a notification in its lifecycle.
///
/// The status tracks where a notification is in its processing flow. Notifications
/// typically progress: Pending → Viewed → (Responded|Dismissed). Ephemeral
/// notifications may transition to Expired.
///
/// # JSON Representation
///
/// Status values serialize as lowercase strings:
/// ```json
/// "pending", "viewed", "responded", "dismissed", "expired"
/// ```
///
/// # State Transitions
///
/// - **Pending** → Viewed: User opens notification
/// - **Viewed** → Responded: User provides response (if required)
/// - **Viewed** → Dismissed: User dismisses notification
/// - **Any** → Expired: Ephemeral notification times out
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::NotificationStatus;
///
/// // Parse from string
/// let status: NotificationStatus = "pending".parse().unwrap();
/// assert_eq!(status, NotificationStatus::Pending);
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    /// Notification is pending user action.
    ///
    /// Initial state for all new notifications. Indicates the user has not
    /// yet interacted with the notification.
    Pending,
    /// User has viewed the notification.
    ///
    /// The notification has been displayed to the user but no further action
    /// has been taken.
    Viewed,
    /// User has responded to the notification.
    ///
    /// Terminal state for notifications that require a response. The response
    /// text is stored in the `response` field.
    Responded,
    /// Notification has been dismissed.
    ///
    /// Terminal state indicating the user has acknowledged the notification
    /// without providing a response (even if one was requested).
    Dismissed,
    /// Notification has expired.
    ///
    /// Terminal state for ephemeral notifications that exceeded their timeout.
    /// Only applicable to notifications with `NotificationLifetime::Ephemeral`.
    Expired,
}

impl std::str::FromStr for NotificationStatus {
    type Err = anyhow::Error;

    /// Parse a status from a string (case-insensitive).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agentd_cli::types::NotificationStatus;
    ///
    /// assert_eq!("pending".parse::<NotificationStatus>().unwrap(), NotificationStatus::Pending);
    /// assert_eq!("RESPONDED".parse::<NotificationStatus>().unwrap(), NotificationStatus::Responded);
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(NotificationStatus::Pending),
            "viewed" => Ok(NotificationStatus::Viewed),
            "responded" => Ok(NotificationStatus::Responded),
            "dismissed" => Ok(NotificationStatus::Dismissed),
            "expired" => Ok(NotificationStatus::Expired),
            _ => Err(anyhow::anyhow!("Invalid status: {s}")),
        }
    }
}

/// A complete notification with all metadata.
///
/// This is the main type returned by the API when fetching notifications.
/// It includes all fields from the create request plus system-managed fields
/// like ID, timestamps, and status.
///
/// # Examples
///
/// ```rust,no_run
/// # use agentd_cli::client::ApiClient;
/// # use agentd_cli::types::Notification;
/// # async fn example() -> anyhow::Result<()> {
/// let client = ApiClient::new("http://localhost:3000".to_string());
/// let notifications: Vec<Notification> = client.get("/notifications").await?;
///
/// for notification in notifications {
///     println!("{}: {} ({})", notification.priority, notification.title, notification.status);
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier for the notification
    pub id: Uuid,
    /// Source that created the notification
    pub source: NotificationSource,
    /// Lifetime strategy (persistent or ephemeral)
    pub lifetime: NotificationLifetime,
    /// Priority level determining urgency
    pub priority: NotificationPriority,
    /// Current status in the notification lifecycle
    pub status: NotificationStatus,
    /// Short, descriptive title
    pub title: String,
    /// Detailed message content
    pub message: String,
    /// Whether the notification requires a user response
    pub requires_response: bool,
    /// User's response text (if provided)
    pub response: Option<String>,
    /// UTC timestamp when the notification was created
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when the notification was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating a new notification.
///
/// This type is serialized to JSON and sent in POST requests to create
/// notifications. The API will generate an ID and timestamps.
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::*;
///
/// let request = CreateNotificationRequest {
///     source: NotificationSource::System,
///     lifetime: NotificationLifetime::Persistent,
///     priority: NotificationPriority::High,
///     title: "Build Failed".to_string(),
///     message: "Tests failed on main branch".to_string(),
///     requires_response: true,
/// };
/// ```
///
/// # CLI Usage
///
/// ```bash
/// agentd notify create \
///   --source system \
///   --lifetime persistent \
///   --priority high \
///   --title "Build Failed" \
///   --message "Tests failed on main branch" \
///   --requires-response
/// ```
#[derive(Debug, Serialize)]
pub struct CreateNotificationRequest {
    /// Source that is creating the notification
    pub source: NotificationSource,
    /// Lifetime strategy for the notification
    pub lifetime: NotificationLifetime,
    /// Priority level
    pub priority: NotificationPriority,
    /// Short, descriptive title
    pub title: String,
    /// Detailed message content
    pub message: String,
    /// Whether the notification requires a user response
    pub requires_response: bool,
}

/// Request payload for updating an existing notification.
///
/// This type is serialized to JSON and sent in PUT requests. Only non-None
/// fields are included in the serialized output, allowing partial updates.
///
/// # Examples
///
/// ```rust
/// use agentd_cli::types::*;
///
/// // Update status only
/// let request = UpdateNotificationRequest {
///     status: Some(NotificationStatus::Viewed),
///     response: None,
/// };
///
/// // Provide a response
/// let request = UpdateNotificationRequest {
///     status: None,
///     response: Some("I've fixed the issue".to_string()),
/// };
/// ```
///
/// # CLI Usage
///
/// ```bash
/// # Respond to a notification
/// agentd notify respond <notification-id> "This is my response"
/// ```
#[derive(Debug, Serialize)]
pub struct UpdateNotificationRequest {
    /// Optional status update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NotificationStatus>,
    /// Optional response text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    #[test]
    fn test_notification_priority_from_str() {
        assert_eq!("low".parse::<NotificationPriority>().unwrap(), NotificationPriority::Low);
        assert_eq!("normal".parse::<NotificationPriority>().unwrap(), NotificationPriority::Normal);
        assert_eq!("high".parse::<NotificationPriority>().unwrap(), NotificationPriority::High);
        assert_eq!("urgent".parse::<NotificationPriority>().unwrap(), NotificationPriority::Urgent);
    }

    #[test]
    fn test_notification_priority_from_str_case_insensitive() {
        assert_eq!("LOW".parse::<NotificationPriority>().unwrap(), NotificationPriority::Low);
        assert_eq!("NoRmAl".parse::<NotificationPriority>().unwrap(), NotificationPriority::Normal);
        assert_eq!("HIGH".parse::<NotificationPriority>().unwrap(), NotificationPriority::High);
        assert_eq!("URGENT".parse::<NotificationPriority>().unwrap(), NotificationPriority::Urgent);
    }

    #[test]
    fn test_notification_priority_from_str_invalid() {
        assert!("invalid".parse::<NotificationPriority>().is_err());
        assert!("medium".parse::<NotificationPriority>().is_err());
        assert!("".parse::<NotificationPriority>().is_err());
    }

    #[test]
    fn test_notification_priority_ordering() {
        assert!(NotificationPriority::Low < NotificationPriority::Normal);
        assert!(NotificationPriority::Normal < NotificationPriority::High);
        assert!(NotificationPriority::High < NotificationPriority::Urgent);
        assert!(NotificationPriority::Low < NotificationPriority::Urgent);
    }

    #[test]
    fn test_notification_status_from_str() {
        assert_eq!("pending".parse::<NotificationStatus>().unwrap(), NotificationStatus::Pending);
        assert_eq!("viewed".parse::<NotificationStatus>().unwrap(), NotificationStatus::Viewed);
        assert_eq!(
            "responded".parse::<NotificationStatus>().unwrap(),
            NotificationStatus::Responded
        );
        assert_eq!(
            "dismissed".parse::<NotificationStatus>().unwrap(),
            NotificationStatus::Dismissed
        );
        assert_eq!("expired".parse::<NotificationStatus>().unwrap(), NotificationStatus::Expired);
    }

    #[test]
    fn test_notification_status_from_str_case_insensitive() {
        assert_eq!("PENDING".parse::<NotificationStatus>().unwrap(), NotificationStatus::Pending);
        assert_eq!("ViEwEd".parse::<NotificationStatus>().unwrap(), NotificationStatus::Viewed);
    }

    #[test]
    fn test_notification_status_from_str_invalid() {
        assert!("invalid".parse::<NotificationStatus>().is_err());
        assert!("active".parse::<NotificationStatus>().is_err());
        assert!("".parse::<NotificationStatus>().is_err());
    }

    #[test]
    fn test_notification_source_serialization() {
        let system = NotificationSource::System;
        let json = serde_json::to_string(&system).unwrap();
        assert_eq!(json, r#"{"type":"system"}"#);

        let ask = NotificationSource::AskService {
            request_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        };
        let json = serde_json::to_string(&ask).unwrap();
        assert_eq!(
            json,
            r#"{"type":"ask_service","request_id":"550e8400-e29b-41d4-a716-446655440000"}"#
        );

        let monitor = NotificationSource::MonitorService { alert_type: "test".to_string() };
        let json = serde_json::to_string(&monitor).unwrap();
        assert_eq!(json, r#"{"type":"monitor_service","alert_type":"test"}"#);

        let hook = NotificationSource::AgentHook {
            agent_id: "agent1".to_string(),
            hook_type: "pre-commit".to_string(),
        };
        let json = serde_json::to_string(&hook).unwrap();
        assert_eq!(json, r#"{"type":"agent_hook","agent_id":"agent1","hook_type":"pre-commit"}"#);
    }

    #[test]
    fn test_notification_source_deserialization() {
        let json = r#"{"type":"system"}"#;
        let source: NotificationSource = serde_json::from_str(json).unwrap();
        assert_eq!(source, NotificationSource::System);

        let json = r#"{"type":"ask_service","request_id":"550e8400-e29b-41d4-a716-446655440000"}"#;
        let source: NotificationSource = serde_json::from_str(json).unwrap();
        match source {
            NotificationSource::AskService { request_id } => {
                assert_eq!(
                    request_id,
                    Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
                );
            }
            _ => panic!("Expected AskService"),
        }
    }

    #[test]
    fn test_notification_lifetime_serialization() {
        let persistent = NotificationLifetime::Persistent;
        let json = serde_json::to_string(&persistent).unwrap();
        assert_eq!(json, r#"{"type":"persistent"}"#);

        let expires_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let ephemeral = NotificationLifetime::Ephemeral { expires_at };
        let json = serde_json::to_string(&ephemeral).unwrap();
        assert!(json.contains(r#""type":"ephemeral""#));
        assert!(json.contains(r#""expires_at""#));
    }

    #[test]
    fn test_notification_lifetime_deserialization() {
        let json = r#"{"type":"persistent"}"#;
        let lifetime: NotificationLifetime = serde_json::from_str(json).unwrap();
        assert_eq!(lifetime, NotificationLifetime::Persistent);

        let json = r#"{"type":"ephemeral","expires_at":"2025-01-01T00:00:00Z"}"#;
        let lifetime: NotificationLifetime = serde_json::from_str(json).unwrap();
        match lifetime {
            NotificationLifetime::Ephemeral { expires_at } => {
                assert_eq!(expires_at.year(), 2025);
                assert_eq!(expires_at.month(), 1);
                assert_eq!(expires_at.day(), 1);
            }
            _ => panic!("Expected Ephemeral"),
        }
    }

    #[test]
    fn test_notification_priority_serialization() {
        assert_eq!(serde_json::to_string(&NotificationPriority::Low).unwrap(), r#""low""#);
        assert_eq!(serde_json::to_string(&NotificationPriority::Normal).unwrap(), r#""normal""#);
        assert_eq!(serde_json::to_string(&NotificationPriority::High).unwrap(), r#""high""#);
        assert_eq!(serde_json::to_string(&NotificationPriority::Urgent).unwrap(), r#""urgent""#);
    }

    #[test]
    fn test_notification_priority_deserialization() {
        assert_eq!(
            serde_json::from_str::<NotificationPriority>(r#""low""#).unwrap(),
            NotificationPriority::Low
        );
        assert_eq!(
            serde_json::from_str::<NotificationPriority>(r#""normal""#).unwrap(),
            NotificationPriority::Normal
        );
        assert_eq!(
            serde_json::from_str::<NotificationPriority>(r#""high""#).unwrap(),
            NotificationPriority::High
        );
        assert_eq!(
            serde_json::from_str::<NotificationPriority>(r#""urgent""#).unwrap(),
            NotificationPriority::Urgent
        );
    }

    #[test]
    fn test_notification_status_serialization() {
        assert_eq!(serde_json::to_string(&NotificationStatus::Pending).unwrap(), r#""pending""#);
        assert_eq!(serde_json::to_string(&NotificationStatus::Viewed).unwrap(), r#""viewed""#);
        assert_eq!(
            serde_json::to_string(&NotificationStatus::Responded).unwrap(),
            r#""responded""#
        );
        assert_eq!(
            serde_json::to_string(&NotificationStatus::Dismissed).unwrap(),
            r#""dismissed""#
        );
        assert_eq!(serde_json::to_string(&NotificationStatus::Expired).unwrap(), r#""expired""#);
    }

    #[test]
    fn test_notification_status_deserialization() {
        assert_eq!(
            serde_json::from_str::<NotificationStatus>(r#""pending""#).unwrap(),
            NotificationStatus::Pending
        );
        assert_eq!(
            serde_json::from_str::<NotificationStatus>(r#""viewed""#).unwrap(),
            NotificationStatus::Viewed
        );
    }

    #[test]
    fn test_notification_serialization() {
        let notification = Notification {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::High,
            status: NotificationStatus::Pending,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: true,
            response: None,
            created_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
        };

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains(r#""id":"550e8400-e29b-41d4-a716-446655440000""#));
        assert!(json.contains(r#""title":"Test""#));
        assert!(json.contains(r#""priority":"high""#));
        assert!(json.contains(r#""requires_response":true"#));
    }

    #[test]
    fn test_notification_deserialization() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "system"},
            "lifetime": {"type": "persistent"},
            "priority": "high",
            "status": "pending",
            "title": "Test",
            "message": "Test message",
            "requires_response": true,
            "response": null,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z"
        }"#;

        let notification: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(
            notification.id,
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        assert_eq!(notification.title, "Test");
        assert_eq!(notification.priority, NotificationPriority::High);
        assert_eq!(notification.status, NotificationStatus::Pending);
        assert!(notification.requires_response);
        assert_eq!(notification.response, None);
    }

    #[test]
    fn test_notification_with_response() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "source": {"type": "system"},
            "lifetime": {"type": "persistent"},
            "priority": "normal",
            "status": "responded",
            "title": "Test",
            "message": "Test message",
            "requires_response": true,
            "response": "My response",
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z"
        }"#;

        let notification: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(notification.response, Some("My response".to_string()));
        assert_eq!(notification.status, NotificationStatus::Responded);
    }

    #[test]
    fn test_create_notification_request_serialization() {
        let request = CreateNotificationRequest {
            source: NotificationSource::System,
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::High,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: false,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""title":"Test""#));
        assert!(json.contains(r#""priority":"high""#));
        assert!(json.contains(r#""requires_response":false"#));
    }

    #[test]
    fn test_update_notification_request_serialization_with_status() {
        let request =
            UpdateNotificationRequest { status: Some(NotificationStatus::Viewed), response: None };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""status":"viewed""#));
        assert!(!json.contains(r#""response""#));
    }

    #[test]
    fn test_update_notification_request_serialization_with_response() {
        let request =
            UpdateNotificationRequest { status: None, response: Some("My response".to_string()) };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""response":"My response""#));
        assert!(!json.contains(r#""status""#));
    }

    #[test]
    fn test_update_notification_request_serialization_empty() {
        let request = UpdateNotificationRequest { status: None, response: None };

        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "{}");
    }
}
