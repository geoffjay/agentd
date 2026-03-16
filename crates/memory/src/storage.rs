//! SeaORM-based persistent storage for memory entries.
//!
//! Provides the [`MemoryStorage`] backend that persists memory entries to an
//! SQLite database using SeaORM entities and a migration-managed schema.
//!
//! # Database Location
//!
//! - Linux: `~/.local/share/agentd-memory/memory.db`
//! - macOS: `~/Library/Application Support/agentd-memory/memory.db`
//!
//! # Schema
//!
//! Managed by [`crate::migration::Migrator`].  See
//! `migration/m20260313_000001_create_memory_entries.rs` for the full
//! column list.
//!
//! # Examples
//!
//! ```no_run
//! use memory::storage::MemoryStorage;
//! use memory::types::{CreateMemoryRequest, MemoryType, VisibilityLevel};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let storage = MemoryStorage::new().await?;
//!
//!     let memory = storage.create(CreateMemoryRequest {
//!         content: "The sky is blue".to_string(),
//!         memory_type: MemoryType::Information,
//!         tags: vec![],
//!         created_by: "agent-1".to_string(),
//!         visibility: VisibilityLevel::Private,
//!         shared_with: vec![],
//!         references: vec![],
//!     }).await?;
//!
//!     println!("Stored memory: {}", memory.id);
//!     Ok(())
//! }
//! ```

use crate::{
    entity::memory_entry as mem_entity,
    error::{StoreError, StoreResult},
    migration::Migrator,
    types::{CreateMemoryRequest, Memory, MemoryType, VisibilityLevel},
};
use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, Set,
};
use sea_orm_migration::prelude::MigratorTrait;
use std::path::{Path, PathBuf};

/// Persistent storage backend for memory entries using SeaORM + SQLite.
///
/// This struct provides a thread-safe, async interface to a SQLite database.
/// [`DatabaseConnection`] is `Clone + Send + Sync`.
#[derive(Clone)]
pub struct MemoryStorage {
    db: DatabaseConnection,
}

impl MemoryStorage {
    /// Returns the platform-specific database file path.
    ///
    /// - **Linux**: `~/.local/share/agentd-memory/memory.db`
    /// - **macOS**: `~/Library/Application Support/agentd-memory/memory.db`
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-memory", "memory.db")
    }

    /// Creates a new storage instance with the default database path.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates a new storage instance connected to `db_path`.
    ///
    /// The file is created if it does not exist, and all pending SeaORM
    /// migrations are applied before returning.
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Inserts a new memory entry and returns the populated [`Memory`] record.
    pub async fn create(&self, request: CreateMemoryRequest) -> StoreResult<Memory> {
        let id = Memory::generate_id();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let tags_json = serde_json::to_string(&request.tags)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let shared_with_json = serde_json::to_string(&request.shared_with)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let refs_json = serde_json::to_string(&request.references)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;

        let model = mem_entity::ActiveModel {
            id: Set(id.clone()),
            content: Set(request.content.clone()),
            memory_type: Set(request.memory_type.to_string()),
            tags: Set(tags_json),
            created_by: Set(request.created_by.clone()),
            owner: Set(None),
            visibility: Set(request.visibility.to_string()),
            shared_with: Set(shared_with_json),
            refs: Set(refs_json),
            created_at: Set(now_str.clone()),
            updated_at: Set(now_str),
        };

        mem_entity::Entity::insert(model)
            .exec(&self.db)
            .await
            .map_err(|e| StoreError::QueryFailed(e.to_string()))?;

        Ok(Memory {
            id,
            content: request.content,
            memory_type: request.memory_type,
            tags: request.tags,
            created_by: request.created_by,
            owner: None,
            created_at: now,
            updated_at: now,
            visibility: request.visibility,
            shared_with: request.shared_with,
            references: request.references,
        })
    }

    /// Retrieves a single memory entry by its ID.
    ///
    /// Returns `None` when no record with the given `id` exists.
    pub async fn get(&self, id: &str) -> StoreResult<Option<Memory>> {
        let model = mem_entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::QueryFailed(e.to_string()))?;

        match model {
            Some(m) => Ok(Some(model_to_memory(m)?)),
            None => Ok(None),
        }
    }

    /// Permanently deletes a memory entry by ID.
    ///
    /// Returns `true` if a record was deleted, `false` if it was not found.
    pub async fn delete(&self, id: &str) -> StoreResult<bool> {
        let result = mem_entity::Entity::delete_many()
            .filter(mem_entity::Column::Id.eq(id))
            .exec(&self.db)
            .await
            .map_err(|e| StoreError::QueryFailed(e.to_string()))?;

        Ok(result.rows_affected > 0)
    }

    /// Lists all memory entries, optionally filtered by creator (newest first).
    pub async fn list(&self, created_by: Option<&str>) -> StoreResult<Vec<Memory>> {
        let mut query =
            mem_entity::Entity::find().order_by(mem_entity::Column::CreatedAt, Order::Desc);

        if let Some(actor) = created_by {
            query = query.filter(mem_entity::Column::CreatedBy.eq(actor));
        }

        let models: Vec<mem_entity::Model> =
            query.all(&self.db).await.map_err(|e| StoreError::QueryFailed(e.to_string()))?;

        models.into_iter().map(model_to_memory).collect()
    }

    /// Updates the visibility and optional share list of a memory entry.
    ///
    /// Returns the updated [`Memory`] record.
    pub async fn update_visibility(
        &self,
        id: &str,
        visibility: VisibilityLevel,
        shared_with: Option<Vec<String>>,
    ) -> StoreResult<Memory> {
        use sea_orm::sea_query::Expr;

        let now = Utc::now().to_rfc3339();
        let shared_list = shared_with.unwrap_or_default();
        let shared_with_json = serde_json::to_string(&shared_list)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;

        let result = mem_entity::Entity::update_many()
            .col_expr(mem_entity::Column::Visibility, Expr::value(visibility.to_string()))
            .col_expr(mem_entity::Column::SharedWith, Expr::value(shared_with_json))
            .col_expr(mem_entity::Column::UpdatedAt, Expr::value(now))
            .filter(mem_entity::Column::Id.eq(id))
            .exec(&self.db)
            .await
            .map_err(|e| StoreError::QueryFailed(e.to_string()))?;

        if result.rows_affected == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }

        self.get(id).await?.ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    /// Returns `true` when the database connection is operational.
    pub async fn health_check(&self) -> StoreResult<bool> {
        let _: Vec<mem_entity::Model> = mem_entity::Entity::find()
            .limit(1)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::QueryFailed(e.to_string()))?;
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw entity [`mem_entity::Model`] into the domain [`Memory`] type.
fn model_to_memory(model: mem_entity::Model) -> StoreResult<Memory> {
    let memory_type: MemoryType =
        model.memory_type.parse().map_err(|e: String| StoreError::InvalidData(e))?;

    let visibility: VisibilityLevel =
        model.visibility.parse().map_err(|e: String| StoreError::InvalidData(e))?;

    let tags: Vec<String> =
        serde_json::from_str(&model.tags).map_err(|e| StoreError::InvalidData(e.to_string()))?;

    let shared_with: Vec<String> = serde_json::from_str(&model.shared_with)
        .map_err(|e| StoreError::InvalidData(e.to_string()))?;

    let references: Vec<String> =
        serde_json::from_str(&model.refs).map_err(|e| StoreError::InvalidData(e.to_string()))?;

    let created_at = chrono::DateTime::parse_from_rfc3339(&model.created_at)
        .map_err(|e| StoreError::InvalidData(e.to_string()))?
        .with_timezone(&chrono::Utc);

    let updated_at = chrono::DateTime::parse_from_rfc3339(&model.updated_at)
        .map_err(|e| StoreError::InvalidData(e.to_string()))?
        .with_timezone(&chrono::Utc);

    Ok(Memory {
        id: model.id,
        content: model.content,
        memory_type,
        tags,
        created_by: model.created_by,
        owner: model.owner,
        created_at,
        updated_at,
        visibility,
        shared_with,
        references,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryType, VisibilityLevel};
    use tempfile::TempDir;

    async fn create_test_storage() -> (MemoryStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = MemoryStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    fn sample_request() -> CreateMemoryRequest {
        CreateMemoryRequest {
            content: "The sky is blue".to_string(),
            memory_type: MemoryType::Information,
            tags: vec!["nature".to_string()],
            created_by: "agent-1".to_string(),
            visibility: VisibilityLevel::Private,
            shared_with: vec![],
            references: vec![],
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let (storage, _temp) = create_test_storage().await;
        let memory = storage.create(sample_request()).await.unwrap();

        assert!(memory.id.starts_with("mem_"));
        assert_eq!(memory.content, "The sky is blue");

        let retrieved = storage.get(&memory.id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, memory.id);
        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.memory_type, MemoryType::Information);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let (storage, _temp) = create_test_storage().await;
        let result = storage.get("mem_nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _temp) = create_test_storage().await;
        let memory = storage.create(sample_request()).await.unwrap();
        let id = memory.id.clone();

        let deleted = storage.delete(&id).await.unwrap();
        assert!(deleted);

        let retrieved = storage.get(&id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let (storage, _temp) = create_test_storage().await;
        let deleted = storage.delete("mem_nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_list_all() {
        let (storage, _temp) = create_test_storage().await;

        storage.create(sample_request()).await.unwrap();
        storage
            .create(CreateMemoryRequest {
                content: "Water is wet".to_string(),
                created_by: "agent-2".to_string(),
                ..sample_request()
            })
            .await
            .unwrap();

        let all = storage.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_filtered_by_creator() {
        let (storage, _temp) = create_test_storage().await;

        storage.create(sample_request()).await.unwrap();
        storage
            .create(CreateMemoryRequest {
                content: "Water is wet".to_string(),
                created_by: "agent-2".to_string(),
                ..sample_request()
            })
            .await
            .unwrap();

        let agent1_memories = storage.list(Some("agent-1")).await.unwrap();
        assert_eq!(agent1_memories.len(), 1);
        assert_eq!(agent1_memories[0].created_by, "agent-1");
    }

    #[tokio::test]
    async fn test_update_visibility() {
        let (storage, _temp) = create_test_storage().await;
        let memory = storage.create(sample_request()).await.unwrap();

        let updated = storage
            .update_visibility(
                &memory.id,
                VisibilityLevel::Shared,
                Some(vec!["agent-2".to_string()]),
            )
            .await
            .unwrap();

        assert_eq!(updated.visibility, VisibilityLevel::Shared);
        assert_eq!(updated.shared_with, vec!["agent-2".to_string()]);
    }

    #[tokio::test]
    async fn test_update_visibility_not_found() {
        let (storage, _temp) = create_test_storage().await;
        let result =
            storage.update_visibility("mem_nonexistent", VisibilityLevel::Public, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check() {
        let (storage, _temp) = create_test_storage().await;
        let healthy = storage.health_check().await.unwrap();
        assert!(healthy);
    }

    #[tokio::test]
    async fn test_tags_and_references_round_trip() {
        let (storage, _temp) = create_test_storage().await;
        let req = CreateMemoryRequest {
            content: "Tagged memory".to_string(),
            memory_type: MemoryType::Information,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            created_by: "agent-1".to_string(),
            visibility: VisibilityLevel::Shared,
            shared_with: vec!["agent-3".to_string()],
            references: vec!["mem_0001_aaaaaaaa".to_string()],
        };

        let memory = storage.create(req).await.unwrap();
        let retrieved = storage.get(&memory.id).await.unwrap().unwrap();

        assert_eq!(retrieved.tags, vec!["tag1".to_string(), "tag2".to_string()]);
        assert_eq!(retrieved.shared_with, vec!["agent-3".to_string()]);
        assert_eq!(retrieved.references, vec!["mem_0001_aaaaaaaa".to_string()]);
    }
}
