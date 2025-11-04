use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Source of a notification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationSource {
    /// From an agent hook requiring user input
    AgentHook { agent_id: String, hook_type: String },
    /// From the agentd-ask service
    AskService { request_id: Uuid },
    /// From the agentd-monitor service
    MonitorService { alert_type: String },
    /// System notification
    System,
}

/// Lifetime behavior of a notification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationLifetime {
    /// Ephemeral notifications expire after a timeout
    /// These are typically tied to active processes and can't be acted on after expiration
    Ephemeral {
        expires_at: DateTime<Utc>,
    },
    /// Persistent notifications remain until explicitly dismissed
    /// These typically represent requests that can be responded to at any time
    Persistent,
}

impl NotificationLifetime {
    /// Check if this notification has expired
    pub fn is_expired(&self) -> bool {
        match self {
            NotificationLifetime::Ephemeral { expires_at } => Utc::now() > *expires_at,
            NotificationLifetime::Persistent => false,
        }
    }

    /// Create an ephemeral notification that expires after the given duration
    pub fn ephemeral(duration: chrono::Duration) -> Self {
        NotificationLifetime::Ephemeral {
            expires_at: Utc::now() + duration,
        }
    }
}

/// Priority level for notifications
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Status of a notification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NotificationStatus {
    /// Notification is pending user action
    Pending,
    /// User has viewed the notification
    Viewed,
    /// User has responded to the notification
    Responded,
    /// Notification has been dismissed
    Dismissed,
    /// Notification has expired (only for ephemeral notifications)
    Expired,
}

/// A notification in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier
    pub id: Uuid,

    /// Source of the notification
    pub source: NotificationSource,

    /// Lifetime behavior
    pub lifetime: NotificationLifetime,

    /// Priority level
    pub priority: NotificationPriority,

    /// Current status
    pub status: NotificationStatus,

    /// Title of the notification
    pub title: String,

    /// Message body
    pub message: String,

    /// Whether this notification requires a response
    pub requires_response: bool,

    /// User's response (if any)
    pub response: Option<String>,

    /// When the notification was created
    pub created_at: DateTime<Utc>,

    /// When the notification was last updated
    pub updated_at: DateTime<Utc>,
}

impl Notification {
    /// Create a new notification
    pub fn new(
        source: NotificationSource,
        lifetime: NotificationLifetime,
        priority: NotificationPriority,
        title: String,
        message: String,
        requires_response: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source,
            lifetime,
            priority,
            status: NotificationStatus::Pending,
            title,
            message,
            requires_response,
            response: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this notification is still actionable
    pub fn is_actionable(&self) -> bool {
        matches!(
            self.status,
            NotificationStatus::Pending | NotificationStatus::Viewed
        ) && !self.lifetime.is_expired()
    }

    /// Mark the notification as viewed
    pub fn mark_viewed(&mut self) {
        if self.status == NotificationStatus::Pending {
            self.status = NotificationStatus::Viewed;
            self.updated_at = Utc::now();
        }
    }

    /// Set the response for this notification
    pub fn set_response(&mut self, response: String) -> anyhow::Result<()> {
        if !self.requires_response {
            anyhow::bail!("This notification does not accept responses");
        }
        if !self.is_actionable() {
            anyhow::bail!("This notification is no longer actionable");
        }

        self.response = Some(response);
        self.status = NotificationStatus::Responded;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Dismiss the notification
    pub fn dismiss(&mut self) {
        self.status = NotificationStatus::Dismissed;
        self.updated_at = Utc::now();
    }

    /// Update status based on lifetime expiration
    pub fn update_expiration_status(&mut self) {
        if self.lifetime.is_expired() && self.status == NotificationStatus::Pending {
            self.status = NotificationStatus::Expired;
            self.updated_at = Utc::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_notification_expiry() {
        let lifetime = NotificationLifetime::ephemeral(chrono::Duration::milliseconds(-1));
        assert!(lifetime.is_expired());
    }

    #[test]
    fn test_persistent_notification_never_expires() {
        let lifetime = NotificationLifetime::Persistent;
        assert!(!lifetime.is_expired());
    }

    #[test]
    fn test_notification_creation() {
        let notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Test message".to_string(),
            false,
        );

        assert_eq!(notification.status, NotificationStatus::Pending);
        assert!(notification.is_actionable());
    }

    #[test]
    fn test_notification_response() {
        let mut notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Test message".to_string(),
            true,
        );

        let result = notification.set_response("My response".to_string());
        assert!(result.is_ok());
        assert_eq!(notification.response, Some("My response".to_string()));
        assert_eq!(notification.status, NotificationStatus::Responded);
    }

    #[test]
    fn test_notification_no_response_when_not_required() {
        let mut notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::Persistent,
            NotificationPriority::Normal,
            "Test".to_string(),
            "Test message".to_string(),
            false,
        );

        let result = notification.set_response("My response".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_notification_not_actionable() {
        let mut notification = Notification::new(
            NotificationSource::System,
            NotificationLifetime::ephemeral(chrono::Duration::milliseconds(-1)),
            NotificationPriority::Normal,
            "Test".to_string(),
            "Test message".to_string(),
            true,
        );

        notification.update_expiration_status();
        assert!(!notification.is_actionable());
        assert_eq!(notification.status, NotificationStatus::Expired);
    }
}
