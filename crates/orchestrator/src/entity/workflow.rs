//! SeaORM entity for the `workflows` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `workflows` table.
///
/// JSON columns (`trigger_config`, `tool_policy`) are stored as TEXT and
/// deserialized manually in the storage layer.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "workflows")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    pub agent_id: String,
    pub trigger_type: String,
    /// JSON-serialized [`crate::scheduler::types::TriggerConfig`].
    pub trigger_config: String,
    pub prompt_template: String,
    pub poll_interval_secs: i64,
    /// Stored as INTEGER (0/1); mapped to `bool` in the domain layer.
    pub enabled: i32,
    /// JSON-serialized [`crate::types::ToolPolicy`].
    pub tool_policy: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::dispatch::Entity")]
    Dispatch,
}

impl Related<super::dispatch::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Dispatch.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
