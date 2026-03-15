//! # agentd-memory
//!
//! A memory service that stores, retrieves, and semantically searches agent
//! memory records using LanceDB for vector embeddings and SQLite for metadata.
//!
//! ## Architecture
//!
//! The memory crate is structured as a dual library + binary:
//!
//! - **Library** (`lib.rs`): Public types, traits, client, and storage backends
//!   used by the CLI and other crates.
//! - **Binary** (`main.rs`): HTTP server with REST API endpoints (private `api`
//!   module).
//!
//! ### Modules
//!
//! | Module           | Description                                               |
//! |------------------|-----------------------------------------------------------|
//! | [`types`]        | Core data structures (`Memory`, `MemoryType`, `VisibilityLevel`) |
//! | [`client`]       | HTTP client for consuming the memory service REST API     |
//! | [`store`]        | `VectorStore` and `EmbeddingService` traits + LanceDB backend |
//! | [`config`]       | Embedding provider and LanceDB configuration              |
//! | [`error`]        | Domain error types with API error conversion              |
//! | [`storage`]      | SQLite-backed `MemoryStorage` for metadata persistence via SeaORM |
//! | [`entity`]       | SeaORM entity definitions for the `memory_entries` table  |
//!
//! ## Access Control
//!
//! Memories use a **three-tier visibility model**:
//!
//! | Level     | Who can read                                  |
//! |-----------|-----------------------------------------------|
//! | `public`  | Everyone (including anonymous)                |
//! | `shared`  | Creator, owner, and actors in `shared_with`   |
//! | `private` | Creator and owner only                        |
//!
//! Access control is enforced by [`Memory::is_visible_to`](types::Memory::is_visible_to),
//! which is called during search operations to post-filter results.
//!
//! ## Storage Backends
//!
//! The [`store::VectorStore`] trait abstracts vector-database operations,
//! with [`store::LanceStore`] as the concrete LanceDB implementation.
//! LanceDB is an embedded database (no external server) that stores data
//! in a local directory using Apache Arrow format.
//!
//! The [`storage::MemoryStorage`] provides SQLite-backed metadata persistence
//! via SeaORM, storing memory entries in:
//! - **Linux**: `~/.local/share/agentd-memory/memory.db`
//! - **macOS**: `~/Library/Application Support/agentd-memory/memory.db`
//!
//! ## Embedding Providers
//!
//! The [`store::EmbeddingService`] trait converts text into float vectors:
//!
//! | Provider  | Configuration                                     |
//! |-----------|----------------------------------------------------|
//! | `openai`  | Remote OpenAI API or local Ollama (OpenAI-compatible) |
//! | `none`    | Disabled — all embed calls return errors            |
//!
//! See [`config::EmbeddingConfig`] for environment variables.
//!
//! ## Client Example
//!
//! ```no_run
//! use memory::client::MemoryClient;
//! use memory::types::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = MemoryClient::new("http://localhost:7008");
//!
//! // Create a memory
//! let request = CreateMemoryRequest {
//!     content: "Paris is the capital of France.".to_string(),
//!     created_by: "agent-1".to_string(),
//!     ..Default::default()
//! };
//! let memory = client.create_memory(&request).await?;
//! println!("Created: {}", memory.id);
//!
//! // Semantic search
//! let results = client.search_memories(&SearchRequest {
//!     query: "capital of France".to_string(),
//!     ..Default::default()
//! }).await?;
//! println!("Found {} results", results.total);
//!
//! // Update visibility
//! client.update_visibility(&memory.id, &UpdateVisibilityRequest {
//!     visibility: VisibilityLevel::Shared,
//!     shared_with: Some(vec!["agent-2".to_string()]),
//!     as_actor: None,
//! }).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Store Example
//!
//! ```no_run
//! use memory::config::{EmbeddingConfig, LanceConfig};
//! use memory::store::{create_store, VectorStore};
//! use memory::types::*;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let store = create_store(
//!     &LanceConfig { path: "/tmp/lance".to_string(), table: "memories".to_string() },
//!     &EmbeddingConfig::default(),
//! ).await?;
//! store.initialize().await?;
//!
//! // Use the store directly
//! let all = store.list_all().await?;
//! println!("Total memories: {}", all.len());
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod config;
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
