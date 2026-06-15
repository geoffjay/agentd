//! SeaORM entity for the `dispatch_log` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `dispatch_log` table.
///
/// The composite unique constraint `(workflow_id, source_id)` is enforced by
/// the migration but not represented in the entity model itself.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "dispatch_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub workflow_id: String,
    pub source_id: String,
    pub agent_id: String,
    pub prompt_sent: String,
    pub status: String,
    pub dispatched_at: String,
    pub completed_at: Option<String>,
    /// JSON-serialized `Task` whose variables were rendered into `prompt_sent`.
    /// NULL for records created before task persistence was added.
    pub task_json: Option<String>,

    /// Optional organization UUID for tenant scoping.  NULL when not
    /// scoped (dev/test mode without gateway).
    pub organization_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow::Entity",
        from = "Column::WorkflowId",
        to = "super::workflow::Column::Id"
    )]
    Workflow,
}

impl Related<super::workflow::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workflow.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
