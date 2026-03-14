//! Storage abstractions for the agentd-memory service.
//!
//! Exposes the [`VectorStore`] and [`EmbeddingService`] traits, the concrete
//! embedding providers ([`OpenAIEmbedding`], [`NoOpEmbedding`]), the LanceDB
//! backend ([`LanceStore`]), and the top-level [`create_store`] factory.
//!
//! # Factory usage
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use memory::config::{EmbeddingConfig, LanceConfig};
//! use memory::store::create_store;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let store = create_store(
//!     &LanceConfig { path: "/tmp/test-lance".to_string(), table: "memories".to_string() },
//!     &EmbeddingConfig::default(),
//! ).await?;
//! store.initialize().await?;
//! # Ok(())
//! # }
//! ```

pub mod embedding;
pub mod lance;
pub(crate) mod lance_embedding;
mod traits;

pub use embedding::{create_embedding_service, model_dimension, NoOpEmbedding, OpenAIEmbedding};
pub use lance::LanceStore;
pub use traits::{EmbeddingService, VectorStore};

use std::sync::Arc;

use crate::config::{EmbeddingConfig, LanceConfig};
use crate::error::StoreResult;

/// Build a [`LanceStore`] from `lance_config` and `embedding_config`.
///
/// Creates the embedding service, opens (or creates) the LanceDB directory,
/// and returns an `Arc<dyn VectorStore>` ready for use.
///
/// Call [`VectorStore::initialize`] on the returned store before first use to
/// ensure the table and indexes exist.
pub async fn create_store(
    lance_config: &LanceConfig,
    embedding_config: &EmbeddingConfig,
) -> StoreResult<Arc<dyn VectorStore>> {
    let embedding_service: Arc<dyn EmbeddingService> =
        Arc::from(create_embedding_service(embedding_config)?);

    let store = LanceStore::new(lance_config, embedding_service).await?;
    Ok(Arc::new(store))
}
