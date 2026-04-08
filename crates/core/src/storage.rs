//! Storage wrapper for the core service.
//!
//! [`Storage`] wraps a [`DatabaseConnection`] and runs all pending SeaORM
//! migrations on construction. It is the single entry point for all
//! database access within the core crate.
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_core::storage::Storage;
//! use agentd_common::storage::{get_db_path, create_connection};
//!
//! let db_path = get_db_path("agentd-core", "core.db")?;
//! let db = create_connection(&db_path).await?;
//! let storage = Storage::new(db).await?;
//! ```

use anyhow::Result;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;

use crate::membership_storage::MembershipStorage;
use crate::migration::Migrator;
use crate::organization_storage::OrganizationStorage;

/// Persistent storage backend for the core service, backed by SQLite via SeaORM.
///
/// Holds a [`DatabaseConnection`] that is `Clone + Send + Sync`, so
/// [`Storage`] itself can be cheaply cloned and shared across async tasks.
#[derive(Clone)]
pub struct Storage {
    db: DatabaseConnection,
}

impl Storage {
    /// Creates a new [`Storage`] instance from an existing connection.
    ///
    /// All pending SeaORM migrations are applied before returning.
    pub async fn new(db: DatabaseConnection) -> Result<Self> {
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }

    /// Exposes the underlying [`DatabaseConnection`] for entity-specific
    /// storage implementations within this crate.
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Returns an [`OrganizationStorage`] instance sharing this connection.
    pub fn organizations(&self) -> OrganizationStorage {
        OrganizationStorage::new(self.db.clone())
    }

    /// Returns a [`MembershipStorage`] instance sharing this connection.
    pub fn memberships(&self) -> MembershipStorage {
        MembershipStorage::new(self.db.clone())
    }
}
