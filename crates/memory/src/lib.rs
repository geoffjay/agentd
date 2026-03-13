//! # agentd-memory
//!
//! A memory service that stores, retrieves, and semantically searches agent
//! memory records using LanceDB for vector embeddings.
//!
//! ## Architecture
//!
//! - **Types** ([`types`]): Core data structures (`Memory`, `MemoryType`, `VisibilityLevel`)
//! - **HTTP Client** ([`client`]): Client for making requests to the memory service
//! - **Storage** ([`store`]): Vector store traits and LanceDB backend
//! - **Config** ([`config`]): Embedding and LanceDB configuration
//! - **Errors** ([`error`]): Domain error types
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
//! let request = CreateMemoryRequest {
//!     content: "Paris is the capital of France.".to_string(),
//!     created_by: "agent-1".to_string(),
//!     ..Default::default()
//! };
//!
//! let memory = client.create_memory(&request).await?;
//! println!("Created: {}", memory.id);
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod store;
pub mod types;
