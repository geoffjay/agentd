//! agentd-memory — Semantic memory service for AI agents.
//!
//! This crate provides the memory service that stores and retrieves agent
//! memories using SQLite for metadata and (in a future issue) LanceDB for
//! vector embeddings.
//!
//! ## Architecture
//!
//! - **Types** ([`types`]): Core data structures — `Memory`, `MemoryType`,
//!   `VisibilityLevel`, request/response types.
//! - **Store** ([`store`]): `VectorStore` and `EmbeddingService` async traits
//!   that decouple the service from concrete backends.
//! - **Storage** ([`storage`]): SQLite-backed `MemoryStorage` for metadata
//!   persistence via SeaORM.
//! - **Error** ([`error`]): `StoreError` and `StoreResult` types.
//!
//! ## Configuration
//!
//! The service listens on port 17008 by default (dev) or 7008 (production)
//! and stores data in:
//! `~/.local/share/agentd-memory/memory.db` (Linux)
//! `~/Library/Application Support/agentd-memory/memory.db` (macOS)

pub mod api;
pub mod entity;
pub mod error;
pub(crate) mod migration;
pub mod storage;
pub mod store;
pub mod types;

/// Apply all pending SeaORM migrations to the SQLite database at `db_path`.
///
/// Creates the file if it does not exist. Designed for use by `cargo xtask migrate`.
pub async fn apply_migrations_for_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    agentd_common::storage::apply_migrations::<migration::Migrator>(db_path).await
}

/// Return the status of all known migrations for the database at `db_path`.
///
/// Each entry is `(migration_name, is_applied)`. Designed for use by
/// `cargo xtask migrate-status`.
pub async fn migration_status_for_path(
    db_path: &std::path::Path,
) -> anyhow::Result<Vec<(String, bool)>> {
    agentd_common::storage::migration_status::<migration::Migrator>(db_path).await
}
