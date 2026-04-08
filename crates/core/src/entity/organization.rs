//! SeaORM entity for the `organizations` table.
//!
//! Full schema (including relations to memberships) is defined in issue #238.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `organizations` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
