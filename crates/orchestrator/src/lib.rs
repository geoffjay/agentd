//! agentd-orchestrator — Agent lifecycle management and workflow orchestration.
//!
//! This crate provides the orchestrator service that manages AI agent processes,
//! implements the Claude Code SDK WebSocket protocol, schedules autonomous
//! workflows, and enforces tool policies.
//!
//! # Modules
//!
//! - [`api`] — REST API handlers and router (agents, workflows, approvals, policies)
//! - [`approvals`] — In-memory registry for human-in-the-loop tool approval requests
//! - [`client`] — Typed HTTP client (`OrchestratorClient`) for consuming the REST API
//! - [`manager`] — Agent lifecycle: spawn, reconcile, terminate tmux sessions
//! - [`scheduler`] — Autonomous workflow scheduling with GitHub issue polling
//! - [`storage`] — SQLite persistence for agent and workflow state
//! - [`types`] — Domain types: `Agent`, `AgentConfig`, `ToolPolicy`, etc.
//! - [`websocket`] — WebSocket SDK server and real-time monitoring streams
//!
//! # Configuration
//!
//! - **Default port:** 17006 (dev) / 7006 (production)
//! - **Database:** `~/Library/Application Support/agentd-orchestrator/orchestrator.db`
//! - **Environment:** `AGENTD_PORT`, `RUST_LOG`, `AGENTD_LOG_FORMAT`

pub mod api;
pub mod approvals;
pub mod client;
pub mod entity;
pub mod manager;
pub mod message_bridge;
pub(crate) mod migration;
pub mod scheduler;
pub mod storage;
pub mod types;
pub mod websocket;

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
