//! SeaORM entity for the `task_queue` table.

use sea_orm::entity::prelude::*;

/// Task status values for queue entries.
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_PROCESSING: &str = "processing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";
pub const STATUS_DEAD: &str = "dead";

/// SeaORM model for the `task_queue` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "task_queue")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub queue_name: String,
    pub title: String,
    pub body: Option<String>,
    pub priority: i32,
    pub status: String,
    pub visibility_timeout_at: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
