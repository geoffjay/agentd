//! # memory-api
//!
//! The lightweight API surface of the agentd-memory service: domain types,
//! the REST client, and the SQLite metadata schema + migrations.
//!
//! This crate exists so that consumers of the memory *API* (the CLI, TUI,
//! and installer) do not inherit the memory *service's* vector-store
//! dependency graph (lancedb → datafusion → arrow, several hundred crates).
//! The `memory` crate depends on this one and re-exports these modules, so
//! the service's own code and docs see an unchanged module layout.
//!
//! | Module        | Description                                               |
//! |---------------|-----------------------------------------------------------|
//! | [`types`]     | Core data structures (`Memory`, `MemoryType`, `VisibilityLevel`) |
//! | [`client`]    | HTTP client for the memory service REST API               |
//! | [`entity`]    | SeaORM entity definitions for the `memory_entries` table  |
//! | [`migration`] | SeaORM migration runner for the metadata database         |

pub mod client;
pub mod entity;
pub mod migration;
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
