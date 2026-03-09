//! SeaORM entity for the `notifications` table.
//!
//! This module defines the ORM model, active model, column enum, and relation
//! enum for notifications stored in SQLite.

use sea_orm::entity::prelude::*;

/// Database model for a notification row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Source type label (e.g. "System", "AgentHook { … }") — metadata only.
    pub source_type: String,

    /// JSON-serialized [`crate::types::NotificationSource`].
    pub source_data: String,

    /// Lifetime discriminant: `"Ephemeral"` or `"Persistent"`.
    pub lifetime_type: String,

    /// RFC3339 expiry timestamp — only set for `"Ephemeral"` rows.
    pub lifetime_expires_at: Option<String>,

    /// Priority label: `"Low"`, `"Normal"`, `"High"`, or `"Urgent"`.
    pub priority: String,

    /// Status label: `"Pending"`, `"Viewed"`, `"Responded"`, `"Dismissed"`, or `"Expired"`.
    pub status: String,

    /// Short notification title.
    pub title: String,

    /// Full notification message body.
    pub message: String,

    /// Boolean stored as `0` or `1`.
    pub requires_response: i32,

    /// User's textual response — `None` until the user responds.
    pub response: Option<String>,

    /// RFC3339 creation timestamp.
    pub created_at: String,

    /// RFC3339 last-update timestamp.
    pub updated_at: String,
}

/// No foreign-key relations — notifications are a self-contained table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
