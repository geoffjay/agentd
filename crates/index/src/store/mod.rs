//! Vector storage backend for the agentd-index service.
//!
//! This module exposes:
//! - [`CodeStore`] / [`EmbeddingService`] traits for pluggable backends.
//! - [`LanceStore`] — the default LanceDB implementation.
//! - [`OllamaEmbedding`] / [`NoOpEmbedding`] — embedding providers.
//! - [`create_store`] — factory that wires config → ready-to-use store.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use index::config::{EmbeddingConfig, LanceConfig};
//! use index::store::create_store;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let store = create_store(
//!     &LanceConfig::default(),
//!     &EmbeddingConfig::default(),
//! ).await?;
//! store.initialize().await?;
//! # Ok(())
//! # }
//! ```

pub mod embedding;
pub mod error;
pub mod lance;
pub mod traits;

pub use embedding::{create_embedding_service, model_dimension, NoOpEmbedding, OllamaEmbedding};
pub use lance::LanceStore;
pub use traits::{CodeStore, EmbeddingService, SearchResult, StoredChunk};

use std::sync::Arc;

use crate::config::{EmbeddingConfig, LanceConfig};
use crate::store::error::StoreResult;

/// Build a [`LanceStore`] from `lance_config` and `embedding_config`.
///
/// Creates the embedding service, opens (or creates) the LanceDB directory,
/// and returns an `Arc<dyn CodeStore>` ready for use.
///
/// Call [`CodeStore::initialize`] on the returned store before first use.
pub async fn create_store(
    lance_config: &LanceConfig,
    embedding_config: &EmbeddingConfig,
) -> StoreResult<Arc<dyn CodeStore>> {
    let embedding_service: Arc<dyn EmbeddingService> =
        Arc::from(create_embedding_service(embedding_config)?);

    let store = LanceStore::new(lance_config, embedding_service).await?;
    Ok(Arc::new(store))
}
