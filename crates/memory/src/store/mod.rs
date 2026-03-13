//! Storage abstractions for the agentd-memory service.
//!
//! This module exposes the [`VectorStore`] and [`EmbeddingService`] traits
//! that decouple the service from concrete backend implementations.
//! LanceDB-backed implementations will be added in a subsequent issue.

mod traits;

pub use traits::{EmbeddingService, VectorStore};
