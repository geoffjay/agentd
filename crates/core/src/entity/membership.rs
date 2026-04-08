//! SeaORM entity for the `memberships` junction table.
//!
//! Full schema (belongs_to User and Organization) is defined in issue #238.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `memberships` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memberships")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
