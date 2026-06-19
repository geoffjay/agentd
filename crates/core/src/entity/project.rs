//! SeaORM entity for the `projects` table.
//!
//! Projects provide a logical grouping layer below organizations:
//! `org > project > {agents, workflows, rooms, docs}`.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `projects` table.
///
/// The `name` column carries a unique index enforced by the migration.
/// `organization_id` is optional — rows with `NULL` represent legacy data
/// created before multi-tenancy was introduced.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    pub description: Option<String>,
    pub organization_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
