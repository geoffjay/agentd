//! # communicate
//!
//! The agentd-communicate service crate.
//!
//! Provides inter-agent and human-agent communication via rooms, participants,
//! and messages, backed by SQLite and exposed through a REST API with
//! real-time WebSocket streaming.
//!
//! ## Crate structure
//!
//! - [`client`] — HTTP client for calling the communicate service from other
//!   crates (e.g. the orchestrator).
//! - [`types`] — Shared domain types, request/response DTOs used by both the
//!   server and the client.
//!
//! ## Example (client usage)
//!
//! ```no_run
//! use communicate::client::CommunicateClient;
//! use communicate::types::{CreateRoomRequest, RoomType};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = CommunicateClient::from_env();
//!
//! let room = client.create_room(&CreateRoomRequest {
//!     name: "general".to_string(),
//!     topic: None,
//!     description: None,
//!     room_type: RoomType::Group,
//!     created_by: "agent-1".to_string(),
//! }).await?;
//!
//! println!("Created room: {}", room.id);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod types;

// Internal modules — used by the binary (main.rs) via its own `mod` declarations.
// Dead-code lint is suppressed here because the lib target sees them as unused;
// the binary target compiles them independently through main.rs.
#[allow(dead_code)]
pub(crate) mod api;
#[allow(dead_code)]
pub(crate) mod entity;
#[allow(dead_code)]
pub(crate) mod migration;
#[allow(dead_code)]
pub(crate) mod storage;
#[allow(dead_code)]
pub(crate) mod websocket;

// ---------------------------------------------------------------------------
// Migration helpers (used by cargo xtask)
// ---------------------------------------------------------------------------

/// Apply all pending SeaORM migrations for the communicate database at the
/// given path, creating the file if it does not yet exist.
pub async fn apply_migrations_for_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    agentd_common::storage::apply_migrations::<migration::Migrator>(db_path).await
}

/// Return the migration status (name, applied) for every known migration of
/// the communicate database at the given path.
pub async fn migration_status_for_path(
    db_path: &std::path::Path,
) -> anyhow::Result<Vec<(String, bool)>> {
    agentd_common::storage::migration_status::<migration::Migrator>(db_path).await
}
