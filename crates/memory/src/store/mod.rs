//! Storage abstractions for the agentd-memory service.
//!
//! This module exposes the [`VectorStore`] and [`EmbeddingService`] traits
//! that decouple the service from concrete backend implementations, as well
//! as the concrete embedding providers ([`OpenAIEmbedding`], [`NoOpEmbedding`])
//! and the [`create_embedding_service`] factory.

mod traits;
pub mod embedding;

pub use embedding::{create_embedding_service, model_dimension, NoOpEmbedding, OpenAIEmbedding};
pub use traits::{EmbeddingService, VectorStore};
