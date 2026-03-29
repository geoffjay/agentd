//! SeaORM entity for the `questions` table.
//!
//! This module defines the ORM model, active model, column enum, and relation
//! enum for questions stored in SQLite.

use sea_orm::entity::prelude::*;

/// Database model for a question row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "questions")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// The notification ID from the notification service (UUID as TEXT).
    pub notification_id: String,

    /// Check type label (e.g. "tmux_sessions").
    pub check_type: String,

    /// RFC3339 timestamp when the question was asked.
    pub asked_at: String,

    /// Status label: `"Pending"`, `"Answered"`, or `"Expired"`.
    pub status: String,

    /// User's textual answer — `None` until the user responds.
    pub answer: Option<String>,
}

/// No foreign-key relations — questions are a self-contained table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
