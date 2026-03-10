//! SeaORM entity for the `agent_usage_sessions` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `agent_usage_sessions` table.
///
/// Each row represents one usage session for an agent.  An *active* session
/// has `ended_at = NULL`.  Token/cost fields are stored as signed integers or
/// floats and converted to unsigned domain types in the storage layer.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "agent_usage_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub agent_id: String,
    pub session_number: i32,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub total_cost_usd: f64,
    pub num_turns: i64,
    pub duration_ms: i64,
    pub duration_api_ms: i64,
    pub result_count: i32,
    /// RFC 3339 timestamp string.
    pub started_at: String,
    /// RFC 3339 timestamp string; `None` means the session is still active.
    pub ended_at: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
