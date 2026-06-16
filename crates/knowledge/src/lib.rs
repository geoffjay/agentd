//! # knowledge
//!
//! The agentd-knowledge service crate.
//!
//! Provides per-project knowledgebase storage: markdown documents stored on
//! the filesystem, with SQLite metadata, exposed via a REST API.
//!
//! ## Crate structure
//!
//! - [`client`] — HTTP client for calling the knowledge service.
//! - [`types`] — Shared domain types, request/response DTOs.
//! - [`error`] — Error types.

pub mod client;
pub mod error;
pub mod types;

// Internal modules used by the binary.
#[allow(dead_code)]
pub(crate) mod api;
#[allow(dead_code)]
pub(crate) mod entity;
pub(crate) mod fs;
#[allow(dead_code)]
pub(crate) mod migration;
pub(crate) mod storage;

// ---------------------------------------------------------------------------
// Router builder (public, used by integration tests)
// ---------------------------------------------------------------------------

/// Build a fully-configured Axum [`axum::Router`] for the knowledge service,
/// backed by a SQLite database at `db_path` and documents stored under `kb_root`.
pub async fn build_router(
    db_path: &std::path::Path,
    kb_root: &std::path::Path,
) -> anyhow::Result<axum::Router> {
    use std::sync::Arc;
    let storage = Arc::new(storage::KnowledgeStorage::with_path(db_path, kb_root).await?);
    Ok(api::create_router_with_state(storage))
}

// ---------------------------------------------------------------------------
// Migration helpers
// ---------------------------------------------------------------------------

/// Apply all pending SeaORM migrations for the knowledge database at the
/// given path, creating the file if it does not yet exist.
pub async fn apply_migrations_for_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    agentd_common::storage::apply_migrations::<migration::Migrator>(db_path).await
}

/// Return the migration status (name, applied) for every known migration of
/// the knowledge database at the given path.
pub async fn migration_status_for_path(
    db_path: &std::path::Path,
) -> anyhow::Result<Vec<(String, bool)>> {
    agentd_common::storage::migration_status::<migration::Migrator>(db_path).await
}
