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
//! The service consists of three main components:
//!
//! - **Notification Types** ([`notification`]): Core data structures and types
//! - **Storage Backend** ([`storage`]): SQLite-based persistence layer
//! - **REST API** ([`api`]): HTTP endpoints for managing notifications
//!
//! ## Example Usage
//!
//! ```no_run
//! use notify::{notification::*, storage::NotificationStorage, api::create_router};
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
//! The service listens on port 3000 by default and stores data in:
//! `~/.local/share/agentd/notifications.db`

pub mod api;
pub mod notification;
pub mod storage;
