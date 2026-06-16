//! SeaORM entity for the `documents` table.
#![allow(dead_code)]

use sea_orm::entity::prelude::*;

/// Database model for a knowledge document row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Project UUID (no FK — cross-DB, mirrors rooms.project_id).
    pub project_id: String,

    /// Relative path within the project (includes `.md`).
    pub rel_path: String,

    /// Document title.
    pub title: String,

    /// Size of the document body in bytes.
    pub size_bytes: i64,

    /// RFC3339 creation timestamp.
    pub created_at: String,

    /// RFC3339 last-update timestamp.
    pub updated_at: String,

    /// Optional organization UUID for tenant scoping.
    pub organization_id: Option<String>,
}

/// No relations for documents (cross-DB project reference).
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
