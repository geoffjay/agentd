//! SeaORM entity for the `rooms` table.
#![allow(dead_code)]

use sea_orm::entity::prelude::*;

/// Database model for a room row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rooms")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// Unique room name (e.g., "general", "ops-alerts").
    #[sea_orm(unique)]
    pub name: String,

    /// Optional topic line displayed in room headers.
    pub topic: Option<String>,

    /// Optional longer description of the room's purpose.
    pub description: Option<String>,

    /// Room type: `"direct"`, `"group"`, or `"broadcast"`.
    pub room_type: String,

    /// Agent UUID or human identifier of the creator.
    pub created_by: String,

    /// RFC3339 creation timestamp.
    pub created_at: String,

    /// RFC3339 last-update timestamp.
    pub updated_at: String,
}

/// Relations from rooms to participants and messages.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::participant::Entity")]
    Participant,
    #[sea_orm(has_many = "super::message::Entity")]
    Message,
}

impl Related<super::participant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Participant.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Message.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
