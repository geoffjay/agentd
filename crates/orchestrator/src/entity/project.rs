//! SeaORM entity for the `projects` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `projects` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Optional organization UUID for tenant scoping.  NULL when not
    /// scoped (dev/test mode without gateway).
    pub organization_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
