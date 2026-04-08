//! SeaORM entity for the `sessions` table.
//!
//! Full schema is defined in issue #239.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `sessions` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
