//! SeaORM entity for the `memory_entries` table.
//!
//! This module defines the ORM model, active model, column enum, and relation
//! enum for memory entries stored in SQLite.

use sea_orm::entity::prelude::*;

/// Database model for a memory entry row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memory_entries")]
pub struct Model {
    /// Unique memory ID in `mem_<unix_ms>_<8-char-uuid-prefix>` format.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Plain-text content of the memory.
    pub content: String,

    /// Memory type label: `"question"`, `"request"`, or `"information"`.
    pub memory_type: String,

    /// JSON-serialised `Vec<String>` of tags.
    pub tags: String,

    /// Agent or user ID that created this memory.
    pub created_by: String,

    /// Optional owner override (defaults to `created_by` when `None`).
    pub owner: Option<String>,

    /// Visibility label: `"private"`, `"shared"`, or `"public"`.
    pub visibility: String,

    /// JSON-serialised `Vec<String>` of actor IDs this memory is shared with.
    pub shared_with: String,

    /// JSON-serialised `Vec<String>` of referenced memory IDs.
    pub refs: String,

    /// RFC3339 creation timestamp.
    pub created_at: String,

    /// RFC3339 last-update timestamp.
    pub updated_at: String,
}

/// No foreign-key relations — memory entries are a self-contained table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
