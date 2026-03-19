//! SeaORM entity for the `participants` table.
#![allow(dead_code)]

use sea_orm::entity::prelude::*;

/// Database model for a participant row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "participants")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK → rooms.id.
    pub room_id: String,

    /// Agent UUID or human identifier. Unique within a room (see migration constraint).
    pub identifier: String,

    /// Participant kind: `"agent"` or `"human"`.
    pub kind: String,

    /// Display name shown in the UI and captured in messages.
    pub display_name: String,

    /// Role within the room: `"member"`, `"admin"`, or `"observer"`.
    pub role: String,

    /// RFC3339 timestamp when the participant joined.
    pub joined_at: String,
}

/// Relations from participants back to the owning room.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::room::Entity",
        from = "Column::RoomId",
        to = "super::room::Column::Id"
    )]
    Room,
}

impl Related<super::room::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Room.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
