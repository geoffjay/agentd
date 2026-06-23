//! Core service library.
//!
//! The core service is the central authentication and API gateway for agentd.
//! This crate exposes its HTTP router so it can be tested without spawning the
//! full binary.

pub mod api;
pub mod config;
pub mod entity;
pub mod membership_storage;
pub mod middleware;
pub mod migration;
pub mod organization_storage;
pub mod pam_auth;
pub mod project_storage;
pub mod proxy;
pub mod session_storage;
pub mod storage;
pub mod user_storage;

/// Apply all pending core-service migrations to the database at `db_path`.
///
/// Creates the database file if it does not exist.
pub async fn apply_migrations_for_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    agentd_common::storage::apply_migrations::<migration::Migrator>(db_path).await
}

/// Return the status of all known core-service migrations at `db_path`.
///
/// Each entry is `(migration_name, is_applied)`.
pub async fn migration_status_for_path(
    db_path: &std::path::Path,
) -> anyhow::Result<Vec<(String, bool)>> {
    agentd_common::storage::migration_status::<migration::Migrator>(db_path).await
}

/// Roll back core-service migrations at `db_path`.
///
/// * `steps = None` — rolls back **all** applied migrations.
/// * `steps = Some(n)` — rolls back the `n` most-recently-applied migrations.
pub async fn rollback_migrations_for_path(
    db_path: &std::path::Path,
    steps: Option<u32>,
) -> anyhow::Result<()> {
    agentd_common::storage::rollback_migrations::<migration::Migrator>(db_path, steps).await
}
