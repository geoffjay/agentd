//! # agentd-memory
//!
//! A memory service that stores, retrieves, and semantically searches agent
//! memory records using LanceDB for vector embeddings.
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
//! ## Storage Backend
//!
//! The [`store::VectorStore`] trait abstracts vector-database operations,
//! with [`store::LanceStore`] as the concrete LanceDB implementation.
//! LanceDB is an embedded database (no external server) that stores data
//! in a local directory using Apache Arrow format.
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
//! let client = MemoryClient::new("http://localhost:17008");
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
pub mod error;
pub mod store;
pub mod types;
