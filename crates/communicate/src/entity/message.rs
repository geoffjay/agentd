//! SeaORM entity for the `messages` table.
#![allow(dead_code)]

use sea_orm::entity::prelude::*;

/// Database model for a message row.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    /// FK → rooms.id.
    pub room_id: String,

    /// Participant identifier of the sender.
    pub sender_id: String,

    /// Display name captured at send time (denormalised for historical accuracy).
    pub sender_name: String,

    /// Sender kind: `"agent"` or `"human"`.
    pub sender_kind: String,

    /// Message body text.
    pub content: String,

    /// JSON-serialised `HashMap<String, String>` of extensible metadata.
    pub metadata: String,

    /// UUID of the message being replied to — `None` for top-level messages.
    pub reply_to: Option<String>,

    /// Delivery status: `"sent"`, `"delivered"`, or `"read"`.
    pub status: String,

    /// RFC3339 creation timestamp.
    pub created_at: String,
}

/// Relations from messages back to the owning room.
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
