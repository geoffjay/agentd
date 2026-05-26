//! SeaORM entity for the `conversation_events` table.

// Model and Relation are used by SeaORM's derive macros and the storage
// layer; they will be called from the WebSocket handler in a follow-up PR.
#![allow(dead_code)]

use sea_orm::entity::prelude::*;

/// SeaORM model for the `conversation_events` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "conversation_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub agent_id: String,
    pub event_type: String,
    pub session_number: i64,
    pub content: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
    #[sea_orm(default_value = 0)]
    pub seq: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
