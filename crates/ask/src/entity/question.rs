//! SeaORM entity for the `questions` table (redesigned for agent-driven Q&A).
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

    /// The agent that created this question.
    pub agent_id: String,

    /// The workflow ID that triggered this question (UUID as TEXT, optional).
    pub workflow_id: Option<String>,

    /// The dispatch ID that triggered this question (UUID as TEXT, optional).
    pub dispatch_id: Option<String>,

    /// Optional category for filtering (e.g. "health", "productivity", "deployment").
    pub category: Option<String>,

    /// The question text.
    pub question: String,

    /// Additional context for the human (optional).
    pub context: Option<String>,

    /// Priority label: `"low"`, `"normal"`, `"high"`, or `"urgent"`.
    pub priority: String,

    /// Status label: `"Pending"`, `"Answered"`, `"Dismissed"`, or `"Expired"`.
    pub status: String,

    /// User's textual answer — `None` until answered.
    pub answer: Option<String>,

    /// RFC3339 timestamp when the question was asked.
    pub asked_at: String,

    /// RFC3339 timestamp when the question was answered (optional).
    pub answered_at: Option<String>,

    /// RFC3339 timestamp when the question expires (optional).
    pub expires_at: Option<String>,
}

/// No foreign-key relations — questions are a self-contained table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
