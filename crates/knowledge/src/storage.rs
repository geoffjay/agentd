//! Storage layer stub for KB-1.
//!
//! Full implementation lives in KB-2.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Persistent storage backend for the knowledge service.
pub struct KnowledgeStorage {
    #[allow(dead_code)]
    pub(crate) db: sea_orm::DatabaseConnection,
    /// Root directory for document files.
    #[allow(dead_code)]
    pub(crate) root: PathBuf,
}

impl KnowledgeStorage {
    /// Platform-specific database file path.
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-knowledge", "knowledge.db")
    }

    /// Creates a new storage instance with the default database path.
    #[allow(dead_code)]
    pub async fn new(root: &Path) -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path, root).await
    }

    /// Creates a new storage instance connected to `db_path`.
    pub async fn with_path(db_path: &Path, root: &Path) -> Result<Self> {
        use crate::migration::Migrator;
        use sea_orm_migration::prelude::MigratorTrait;
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        std::fs::create_dir_all(root)?;
        Ok(Self { db, root: root.to_path_buf() })
    }
}
