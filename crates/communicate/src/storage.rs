//! SeaORM-based persistent storage for the communicate service.
//!
//! - Linux: `~/.local/share/agentd-communicate/communicate.db`
//! - macOS: `~/Library/Application Support/agentd-communicate/communicate.db`

use crate::entity;
use crate::migration::Migrator;
use crate::types::{
    AddParticipantRequest, CreateMessageRequest, CreateRoomRequest, Message, MessageStatus,
    Participant, ParticipantKind, ParticipantRole, Room, RoomType,
};
use agentd_common::error::ApiError;
use anyhow::Result;
use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, QueryFilter, QueryOrder, QuerySelect, Set};
use sea_orm_migration::prelude::MigratorTrait;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Persistent storage backend for the communicate service using SeaORM + SQLite.
#[derive(Clone)]
pub struct CommunicateStorage {
    pub(crate) db: DatabaseConnection,
}

impl CommunicateStorage {
    /// Platform-specific database file path.
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-communicate", "communicate.db")
    }

    /// Creates a new storage instance with the default database path.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new storage instance connected to `db_path`, running all
    /// pending migrations before returning.
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    // -----------------------------------------------------------------------
    // Room operations
    // -----------------------------------------------------------------------

    /// Creates a new room from the given request.
    ///
    /// Returns a `409 Conflict` (`ApiError::Conflict`) if a room with the same
    /// name already exists.
    pub async fn create_room(&self, req: &CreateRoomRequest) -> Result<Room, ApiError> {
        if req.name.trim().is_empty() {
            return Err(ApiError::InvalidInput("room name must not be empty".to_string()));
        }

        // Check for duplicate name.
        let existing = entity::room::Entity::find()
            .filter(entity::room::Column::Name.eq(req.name.as_str()))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        if existing.is_some() {
            return Err(ApiError::Conflict(format!("a room named '{}' already exists", req.name)));
        }

        let now = chrono::Utc::now();
        let id = Uuid::new_v4();

        let model = entity::room::ActiveModel {
            id: Set(id.to_string()),
            name: Set(req.name.clone()),
            topic: Set(req.topic.clone()),
            description: Set(req.description.clone()),
            room_type: Set(req.room_type.to_string()),
            created_by: Set(req.created_by.clone()),
            created_at: Set(now.to_rfc3339()),
            updated_at: Set(now.to_rfc3339()),
        };

        model.insert(&self.db).await.map_err(|e| ApiError::Internal(e.into()))?;

        let inserted = entity::room::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("room not found after insert")))?;

        model_to_room(inserted).map_err(ApiError::Internal)
    }

    /// Retrieves a room by its UUID.
    pub async fn get_room(&self, id: &Uuid) -> Result<Option<Room>, ApiError> {
        let row = entity::room::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        row.map(|m| model_to_room(m).map_err(ApiError::Internal)).transpose()
    }

    /// Retrieves a room by its unique name.
    #[allow(dead_code)]
    pub async fn get_room_by_name(&self, name: &str) -> Result<Option<Room>, ApiError> {
        let row = entity::room::Entity::find()
            .filter(entity::room::Column::Name.eq(name))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        row.map(|m| model_to_room(m).map_err(ApiError::Internal)).transpose()
    }

    /// Returns a paginated list of all rooms and the total count.
    pub async fn list_rooms(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Room>, usize), ApiError> {
        let total = entity::room::Entity::find()
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))? as usize;

        let rows = entity::room::Entity::find()
            .order_by_asc(entity::room::Column::Name)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let rooms = rows
            .into_iter()
            .map(model_to_room)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        Ok((rooms, total))
    }

    /// Returns a paginated list of rooms filtered by type, and the total count
    /// of rooms matching the filter.
    pub async fn list_rooms_by_type(
        &self,
        room_type: &RoomType,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Room>, usize), ApiError> {
        let type_str = room_type.to_string();

        let total = entity::room::Entity::find()
            .filter(entity::room::Column::RoomType.eq(type_str.as_str()))
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))? as usize;

        let rows = entity::room::Entity::find()
            .filter(entity::room::Column::RoomType.eq(type_str.as_str()))
            .order_by_asc(entity::room::Column::Name)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let rooms = rows
            .into_iter()
            .map(model_to_room)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        Ok((rooms, total))
    }

    /// Updates the mutable fields of a room (topic and/or description).
    ///
    /// Returns `None` if the room does not exist.
    pub async fn update_room(
        &self,
        id: &Uuid,
        topic: Option<String>,
        description: Option<String>,
    ) -> Result<Option<Room>, ApiError> {
        use sea_orm::IntoActiveModel;

        let row = entity::room::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut active = row.into_active_model();
        if topic.is_some() {
            active.topic = Set(topic);
        }
        if description.is_some() {
            active.description = Set(description);
        }
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());

        let updated = active.update(&self.db).await.map_err(|e| ApiError::Internal(e.into()))?;

        let room = model_to_room(updated).map_err(ApiError::Internal)?;
        Ok(Some(room))
    }

    /// Deletes a room by ID. Returns `true` if deleted, `false` if not found.
    pub async fn delete_room(&self, id: &Uuid) -> Result<bool, ApiError> {
        let result = entity::room::Entity::delete_by_id(id.to_string())
            .exec(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(result.rows_affected > 0)
    }

    /// Returns the total number of rooms in the database.
    pub async fn count_rooms(&self) -> Result<usize, ApiError> {
        let count = entity::room::Entity::find()
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;
        Ok(count as usize)
    }

    /// Returns a paginated list of participants in the given room and the total
    /// count.
    pub async fn list_participants_in_room(
        &self,
        room_id: &Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Participant>, usize), ApiError> {
        let id_str = room_id.to_string();

        let total = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(id_str.as_str()))
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))? as usize;

        let rows = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(id_str.as_str()))
            .order_by_asc(entity::participant::Column::JoinedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let participants = rows
            .into_iter()
            .map(model_to_participant)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        Ok((participants, total))
    }

    // -----------------------------------------------------------------------
    // Participant operations
    // -----------------------------------------------------------------------

    /// Adds a participant to a room.
    ///
    /// Returns `ApiError::NotFound` if the room does not exist, or
    /// `ApiError::Conflict` if the identifier is already in the room.
    pub async fn add_participant(
        &self,
        room_id: &Uuid,
        req: &AddParticipantRequest,
    ) -> Result<Participant, ApiError> {
        if req.identifier.trim().is_empty() {
            return Err(ApiError::InvalidInput("identifier must not be empty".to_string()));
        }

        // Verify the room exists.
        let room_exists = entity::room::Entity::find_by_id(room_id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .is_some();

        if !room_exists {
            return Err(ApiError::NotFound);
        }

        // Enforce uniqueness of (room_id, identifier).
        let existing = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .filter(entity::participant::Column::Identifier.eq(req.identifier.as_str()))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        if existing.is_some() {
            return Err(ApiError::Conflict(format!(
                "participant '{}' is already a member of room '{}'",
                req.identifier, room_id
            )));
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let model = entity::participant::ActiveModel {
            id: Set(id.to_string()),
            room_id: Set(room_id.to_string()),
            identifier: Set(req.identifier.clone()),
            kind: Set(req.kind.to_string()),
            display_name: Set(req.display_name.clone()),
            role: Set(req.role.to_string()),
            joined_at: Set(now.to_rfc3339()),
        };

        model.insert(&self.db).await.map_err(|e| ApiError::Internal(e.into()))?;

        let inserted = entity::participant::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .ok_or_else(|| {
                ApiError::Internal(anyhow::anyhow!("participant not found after insert"))
            })?;

        model_to_participant(inserted).map_err(ApiError::Internal)
    }

    /// Retrieves a participant from a room by identifier.
    pub async fn get_participant(
        &self,
        room_id: &Uuid,
        identifier: &str,
    ) -> Result<Option<Participant>, ApiError> {
        let row = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .filter(entity::participant::Column::Identifier.eq(identifier))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        row.map(|m| model_to_participant(m).map_err(ApiError::Internal)).transpose()
    }

    /// Removes a participant from a room by identifier.
    ///
    /// Returns `true` if removed, `false` if not found.
    pub async fn remove_participant(
        &self,
        room_id: &Uuid,
        identifier: &str,
    ) -> Result<bool, ApiError> {
        let result = entity::participant::Entity::delete_many()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .filter(entity::participant::Column::Identifier.eq(identifier))
            .exec(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(result.rows_affected > 0)
    }

    /// Updates a participant's role within a room.
    ///
    /// Returns `None` if the participant is not found.
    pub async fn update_participant_role(
        &self,
        room_id: &Uuid,
        identifier: &str,
        role: ParticipantRole,
    ) -> Result<Option<Participant>, ApiError> {
        use sea_orm::IntoActiveModel;

        let row = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .filter(entity::participant::Column::Identifier.eq(identifier))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut active = row.into_active_model();
        active.role = Set(role.to_string());

        let updated = active.update(&self.db).await.map_err(|e| ApiError::Internal(e.into()))?;

        let participant = model_to_participant(updated).map_err(ApiError::Internal)?;
        Ok(Some(participant))
    }

    /// Returns all rooms a participant (by identifier) belongs to, paginated.
    ///
    /// Ordering is by room name ascending.
    pub async fn get_rooms_for_participant(
        &self,
        identifier: &str,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<Room>, usize), ApiError> {
        // Collect all room IDs for this identifier.
        let participant_rows = entity::participant::Entity::find()
            .filter(entity::participant::Column::Identifier.eq(identifier))
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let room_ids: Vec<String> = participant_rows.into_iter().map(|p| p.room_id).collect();
        let total = room_ids.len();

        let paginated_ids: Vec<String> = room_ids.into_iter().skip(offset).take(limit).collect();

        if paginated_ids.is_empty() {
            return Ok((vec![], total));
        }

        let rows = entity::room::Entity::find()
            .filter(entity::room::Column::Id.is_in(paginated_ids))
            .order_by_asc(entity::room::Column::Name)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let rooms = rows
            .into_iter()
            .map(model_to_room)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        Ok((rooms, total))
    }

    /// Returns the number of participants currently in a room.
    pub async fn count_participants_in_room(&self, room_id: &Uuid) -> Result<usize, ApiError> {
        let count = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))? as usize;
        Ok(count)
    }

    // -----------------------------------------------------------------------
    // Message operations
    // -----------------------------------------------------------------------

    /// Sends a message to a room.
    ///
    /// Validates that:
    /// - Content is non-empty.
    /// - The room exists (`ApiError::NotFound`).
    /// - The sender is a participant in the room (`ApiError::Forbidden`).
    /// - `reply_to`, if present, references an existing message in the same room
    ///   (`ApiError::InvalidInput`).
    pub async fn send_message(
        &self,
        room_id: &Uuid,
        req: &CreateMessageRequest,
    ) -> Result<Message, ApiError> {
        if req.content.trim().is_empty() {
            return Err(ApiError::InvalidInput("message content must not be empty".to_string()));
        }

        // Verify room exists.
        let room_exists = entity::room::Entity::find_by_id(room_id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .is_some();

        if !room_exists {
            return Err(ApiError::NotFound);
        }

        // Verify sender is a participant.
        let is_participant = entity::participant::Entity::find()
            .filter(entity::participant::Column::RoomId.eq(room_id.to_string()))
            .filter(entity::participant::Column::Identifier.eq(req.sender_id.as_str()))
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .is_some();

        if !is_participant {
            return Err(ApiError::Forbidden(format!(
                "'{}' is not a participant in room '{}'",
                req.sender_id, room_id
            )));
        }

        // Validate reply_to references an existing message in the same room.
        if let Some(reply_id) = &req.reply_to {
            let reply_in_room = entity::message::Entity::find_by_id(reply_id.to_string())
                .one(&self.db)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?
                .map(|m| m.room_id == room_id.to_string())
                .unwrap_or(false);

            if !reply_in_room {
                return Err(ApiError::InvalidInput(format!(
                    "reply_to '{}' does not reference a message in this room",
                    reply_id
                )));
            }
        }

        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let metadata_json = serde_json::to_string(&req.metadata).map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("failed to serialize metadata: {}", e))
        })?;

        let model = entity::message::ActiveModel {
            id: Set(id.to_string()),
            room_id: Set(room_id.to_string()),
            sender_id: Set(req.sender_id.clone()),
            sender_name: Set(req.sender_name.clone()),
            sender_kind: Set(req.sender_kind.to_string()),
            content: Set(req.content.clone()),
            metadata: Set(metadata_json),
            reply_to: Set(req.reply_to.map(|u| u.to_string())),
            status: Set(MessageStatus::Sent.to_string()),
            created_at: Set(now.to_rfc3339()),
        };

        model.insert(&self.db).await.map_err(|e| ApiError::Internal(e.into()))?;

        let inserted = entity::message::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("message not found after insert")))?;

        model_to_message(inserted).map_err(ApiError::Internal)
    }

    /// Retrieves a single message by its UUID.
    pub async fn get_message(&self, id: &Uuid) -> Result<Option<Message>, ApiError> {
        let row = entity::message::Entity::find_by_id(id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        row.map(|m| model_to_message(m).map_err(ApiError::Internal)).transpose()
    }

    /// Returns a paginated list of messages in a room, optionally filtered by
    /// timestamp.
    ///
    /// - `before` — only messages created strictly before this timestamp.
    /// - `after`  — only messages created strictly after this timestamp.
    ///
    /// Messages are returned in ascending creation order.
    pub async fn list_messages(
        &self,
        room_id: &Uuid,
        limit: usize,
        offset: usize,
        before: Option<chrono::DateTime<chrono::Utc>>,
        after: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(Vec<Message>, usize), ApiError> {
        let id_str = room_id.to_string();

        let mut count_query = entity::message::Entity::find()
            .filter(entity::message::Column::RoomId.eq(id_str.as_str()));
        let mut list_query = entity::message::Entity::find()
            .filter(entity::message::Column::RoomId.eq(id_str.as_str()));

        if let Some(before_ts) = before {
            let before_str = before_ts.to_rfc3339();
            count_query =
                count_query.filter(entity::message::Column::CreatedAt.lt(before_str.clone()));
            list_query = list_query.filter(entity::message::Column::CreatedAt.lt(before_str));
        }
        if let Some(after_ts) = after {
            let after_str = after_ts.to_rfc3339();
            count_query =
                count_query.filter(entity::message::Column::CreatedAt.gt(after_str.clone()));
            list_query = list_query.filter(entity::message::Column::CreatedAt.gt(after_str));
        }

        let total =
            count_query.count(&self.db).await.map_err(|e| ApiError::Internal(e.into()))? as usize;

        let rows = list_query
            .order_by_asc(entity::message::Column::CreatedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let messages = rows
            .into_iter()
            .map(model_to_message)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        Ok((messages, total))
    }

    /// Deletes a message by ID. Returns `true` if deleted, `false` if not found.
    pub async fn delete_message(&self, id: &Uuid) -> Result<bool, ApiError> {
        let result = entity::message::Entity::delete_by_id(id.to_string())
            .exec(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(result.rows_affected > 0)
    }

    /// Returns the total number of messages in a room.
    pub async fn get_room_message_count(&self, room_id: &Uuid) -> Result<usize, ApiError> {
        let count = entity::message::Entity::find()
            .filter(entity::message::Column::RoomId.eq(room_id.to_string()))
            .count(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))? as usize;
        Ok(count)
    }

    /// Returns the N most recent messages in a room, ordered oldest-first.
    pub async fn get_latest_messages(
        &self,
        room_id: &Uuid,
        count: usize,
    ) -> Result<Vec<Message>, ApiError> {
        // Fetch the latest `count` rows ordered by created_at DESC, then
        // reverse to return them oldest-first.
        let rows = entity::message::Entity::find()
            .filter(entity::message::Column::RoomId.eq(room_id.to_string()))
            .order_by_desc(entity::message::Column::CreatedAt)
            .limit(count as u64)
            .all(&self.db)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let mut messages = rows
            .into_iter()
            .map(model_to_message)
            .collect::<Result<Vec<_>>>()
            .map_err(ApiError::Internal)?;

        messages.reverse();
        Ok(messages)
    }
}

// ---------------------------------------------------------------------------
// Model conversion helpers
// ---------------------------------------------------------------------------

/// Converts a SeaORM room model into the domain `Room` type.
pub fn model_to_room(m: entity::room::Model) -> Result<Room> {
    Ok(Room {
        id: m.id.parse::<Uuid>()?,
        name: m.name,
        topic: m.topic,
        description: m.description,
        room_type: m.room_type.parse::<RoomType>()?,
        created_by: m.created_by,
        created_at: m.created_at.parse::<chrono::DateTime<chrono::Utc>>()?,
        updated_at: m.updated_at.parse::<chrono::DateTime<chrono::Utc>>()?,
    })
}

/// Converts a SeaORM participant model into the domain `Participant` type.
pub fn model_to_participant(m: entity::participant::Model) -> Result<Participant> {
    Ok(Participant {
        id: m.id.parse::<Uuid>()?,
        room_id: m.room_id.parse::<Uuid>()?,
        identifier: m.identifier,
        kind: m.kind.parse::<ParticipantKind>()?,
        display_name: m.display_name,
        role: m.role.parse::<ParticipantRole>()?,
        joined_at: m.joined_at.parse::<chrono::DateTime<chrono::Utc>>()?,
    })
}

/// Converts a SeaORM message model into the domain `Message` type.
pub fn model_to_message(m: entity::message::Model) -> Result<Message> {
    let metadata = serde_json::from_str(&m.metadata).unwrap_or_default();
    let reply_to = m.reply_to.map(|s| s.parse::<Uuid>()).transpose()?;
    Ok(Message {
        id: m.id.parse::<Uuid>()?,
        room_id: m.room_id.parse::<Uuid>()?,
        sender_id: m.sender_id,
        sender_name: m.sender_name,
        sender_kind: m.sender_kind.parse()?,
        content: m.content,
        metadata,
        reply_to,
        status: m.status.parse()?,
        created_at: m.created_at.parse::<chrono::DateTime<chrono::Utc>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoomType;
    use tempfile::TempDir;

    async fn create_test_storage() -> (CommunicateStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = CommunicateStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    fn make_create_req(name: &str) -> CreateRoomRequest {
        CreateRoomRequest {
            name: name.to_string(),
            topic: Some("Test topic".to_string()),
            description: Some("Test description".to_string()),
            room_type: RoomType::Group,
            created_by: "agent-test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_storage_init() {
        let (_storage, _temp) = create_test_storage().await;
    }

    #[tokio::test]
    async fn test_storage_clone() {
        let (storage, _temp) = create_test_storage().await;
        let _clone = storage.clone();
    }

    #[tokio::test]
    async fn test_create_and_get_room() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("general");
        let room = storage.create_room(&req).await.unwrap();

        assert_eq!(room.name, "general");
        assert_eq!(room.room_type, RoomType::Group);
        assert_eq!(room.created_by, "agent-test");

        let fetched = storage.get_room(&room.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, room.id);
        assert_eq!(fetched.name, room.name);
    }

    #[tokio::test]
    async fn test_create_duplicate_room_name() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("duplicate");
        storage.create_room(&req).await.unwrap();

        let result = storage.create_room(&req).await;
        assert!(matches!(result, Err(ApiError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_create_room_empty_name() {
        let (storage, _temp) = create_test_storage().await;

        let req = CreateRoomRequest {
            name: "   ".to_string(),
            topic: None,
            description: None,
            room_type: RoomType::Group,
            created_by: "agent-test".to_string(),
        };
        let result = storage.create_room(&req).await;
        assert!(matches!(result, Err(ApiError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_list_rooms_paginated() {
        let (storage, _temp) = create_test_storage().await;

        for i in 0..5 {
            let req = make_create_req(&format!("room-{i:02}"));
            storage.create_room(&req).await.unwrap();
        }

        let (first_page, total) = storage.list_rooms(3, 0).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(first_page.len(), 3);

        let (second_page, total2) = storage.list_rooms(3, 3).await.unwrap();
        assert_eq!(total2, 5);
        assert_eq!(second_page.len(), 2);
    }

    #[tokio::test]
    async fn test_get_room_by_name() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("by-name");
        let created = storage.create_room(&req).await.unwrap();

        let found = storage.get_room_by_name("by-name").await.unwrap().unwrap();
        assert_eq!(found.id, created.id);

        let missing = storage.get_room_by_name("no-such-room").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_update_room() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("updateable");
        let room = storage.create_room(&req).await.unwrap();

        let updated = storage
            .update_room(&room.id, Some("New topic".to_string()), None)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.topic, Some("New topic".to_string()));
        // description unchanged
        assert_eq!(updated.description, room.description);
    }

    #[tokio::test]
    async fn test_update_room_not_found() {
        let (storage, _temp) = create_test_storage().await;

        let missing_id = Uuid::new_v4();
        let result = storage.update_room(&missing_id, None, None).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_room_cascades() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("to-delete");
        let room = storage.create_room(&req).await.unwrap();

        let deleted = storage.delete_room(&room.id).await.unwrap();
        assert!(deleted);

        let fetched = storage.get_room(&room.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_room_not_found() {
        let (storage, _temp) = create_test_storage().await;

        let missing_id = Uuid::new_v4();
        let deleted = storage.delete_room(&missing_id).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_count_rooms() {
        let (storage, _temp) = create_test_storage().await;

        assert_eq!(storage.count_rooms().await.unwrap(), 0);

        storage.create_room(&make_create_req("r1")).await.unwrap();
        storage.create_room(&make_create_req("r2")).await.unwrap();
        assert_eq!(storage.count_rooms().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_list_rooms_by_type() {
        let (storage, _temp) = create_test_storage().await;

        let mut req = make_create_req("direct-room");
        req.room_type = RoomType::Direct;
        storage.create_room(&req).await.unwrap();

        storage.create_room(&make_create_req("group-room")).await.unwrap();

        let (direct_rooms, direct_total) =
            storage.list_rooms_by_type(&RoomType::Direct, 10, 0).await.unwrap();
        assert_eq!(direct_total, 1);
        assert_eq!(direct_rooms.len(), 1);
        assert_eq!(direct_rooms[0].name, "direct-room");

        let (group_rooms, group_total) =
            storage.list_rooms_by_type(&RoomType::Group, 10, 0).await.unwrap();
        assert_eq!(group_total, 1);
        assert_eq!(group_rooms.len(), 1);
    }

    #[tokio::test]
    async fn test_list_participants_in_room_empty() {
        let (storage, _temp) = create_test_storage().await;

        let req = make_create_req("empty-room");
        let room = storage.create_room(&req).await.unwrap();

        let (participants, total) =
            storage.list_participants_in_room(&room.id, 10, 0).await.unwrap();
        assert_eq!(total, 0);
        assert!(participants.is_empty());
    }
}
