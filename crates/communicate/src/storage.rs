//! SeaORM-based persistent storage for the communicate service.
//!
//! Provides the [`CommunicateStorage`] backend that persists data to an SQLite
//! database using SeaORM entities and a migration-managed schema.
//!
//! # Database Location
//!
//! - Linux: `~/.local/share/agentd-communicate/communicate.db`
//! - macOS: `~/Library/Application Support/agentd-communicate/communicate.db`
//!
//! # Schema
//!
//! Managed by [`crate::migration::Migrator`].

use crate::migration::Migrator;
use anyhow::Result;
use sea_orm::DatabaseConnection;
use sea_orm_migration::prelude::MigratorTrait;
use std::path::{Path, PathBuf};

/// Persistent storage backend for the communicate service using SeaORM + SQLite.
///
/// [`DatabaseConnection`] is `Clone + Send + Sync`.
#[derive(Clone)]
pub struct CommunicateStorage {
    #[allow(dead_code)]
    db: DatabaseConnection,
}

impl CommunicateStorage {
    /// Gets the platform-specific database file path.
    ///
    /// - **Linux**: `~/.local/share/agentd-communicate/communicate.db`
    /// - **macOS**: `~/Library/Application Support/agentd-communicate/communicate.db`
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-communicate", "communicate.db")
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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_storage() -> (CommunicateStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = CommunicateStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_storage_init() {
        let (_storage, _temp) = create_test_storage().await;
        // Storage initializes without error and migrations run successfully.
    }

    #[tokio::test]
    async fn test_storage_clone() {
        let (storage, _temp) = create_test_storage().await;
        let _clone = storage.clone();
    }
}
