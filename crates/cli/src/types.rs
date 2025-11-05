//! Type definitions specific to the CLI.
//!
//! This module re-exports notification types from the notify crate and adds
//! CLI-specific request/response types.

// Re-export notification types from the notify crate
pub use notify::notification::{
    Notification, NotificationLifetime, NotificationPriority, NotificationSource,
    NotificationStatus,
};

use serde::{Deserialize, Serialize};

/// Request body for creating a new notification.
///
/// This type is used by the CLI when making POST requests to create notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationRequest {
    pub source: NotificationSource,
    pub lifetime: NotificationLifetime,
    pub priority: NotificationPriority,
    pub title: String,
    pub message: String,
    pub requires_response: bool,
}

/// Request body for updating an existing notification.
///
/// Both fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotificationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NotificationStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
}

