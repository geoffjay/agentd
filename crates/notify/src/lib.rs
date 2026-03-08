//! # agentd-notify
//!
//! A notification service that manages persistent and ephemeral notifications with a REST API.
//!
//! ## Overview
//!
//! The `agentd-notify` service provides a centralized notification system that can:
//! - Store notifications with different priority levels
//! - Manage both ephemeral (time-limited) and persistent notifications
//! - Track notification status (pending, viewed, responded, dismissed, expired)
//! - Filter and query notifications
//! - Provide a REST API for notification management
//!
//! ## Architecture
//!
//! The service consists of four main components:
//!
//! - **Notification Types** ([`types`]): Core data structures and types
//! - **HTTP Client** ([`client`]): Client for making requests to the notification service
//! - **Storage Backend** ([`storage`]): SQLite-based persistence layer
//! - **REST API** ([`api`]): HTTP endpoints for managing notifications
//!
//! ## Example Usage (Service)
//!
//! ```no_run
//! use notify::{types::*, storage::NotificationStorage, api::create_router};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize storage
//!     let storage = NotificationStorage::new().await?;
//!
//!     // Create a notification
//!     let notification = Notification::new(
//!         NotificationSource::System,
//!         NotificationLifetime::Persistent,
//!         NotificationPriority::High,
//!         "Test".to_string(),
//!         "Hello, world!".to_string(),
//!         false,
//!     );
//!
//!     // Store it
//!     storage.add(&notification).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Example Usage (Client)
//!
//! ```no_run
//! use notify::client::NotifyClient;
//! use notify::types::*;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a client
//!     let client = NotifyClient::new("http://localhost:7004");
//!
//!     // List notifications
//!     let response = client.list_notifications().await?;
//!     println!("Found {} notifications", response.total);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Multiple Priority Levels**: Low, Normal, High, Urgent
//! - **Flexible Lifetimes**: Persistent or ephemeral with expiration
//! - **Status Tracking**: Pending, viewed, responded, dismissed, expired
//! - **Rich Querying**: Filter by status, list actionable items, view history
//! - **REST API**: Full HTTP API for external integration
//!
//! ## Configuration
//!
//! The service listens on port 17004 by default (dev) or 7004 (production) and stores data in:
//! `~/.local/share/agentd/notifications.db`

pub mod api;
pub mod client;
pub mod entity;
pub(crate) mod migration;
pub mod notification;
pub mod storage;
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
