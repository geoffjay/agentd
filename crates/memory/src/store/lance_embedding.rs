//! Bridge between agentd-memory's [`EmbeddingService`] and LanceDB's
//! [`EmbeddingFunction`] trait.
//!
//! LanceDB's embedding integration requires a synchronous trait while
//! [`EmbeddingService`] is async.  [`AgentdEmbeddingBridge`] handles this by
//! bridging to the current Tokio runtime via
//! `tokio::runtime::Handle::current().block_on(...)`.

use std::borrow::Cow;
use std::sync::Arc;

use arrow_array::types::Float32Type;
use arrow_array::{Array, ArrayRef, FixedSizeListArray, StringArray};
use arrow_schema::DataType;
use lancedb::embeddings::EmbeddingFunction;

use crate::store::EmbeddingService;

/// Bridges agentd-memory's [`EmbeddingService`] to LanceDB's
/// [`EmbeddingFunction`] trait.
///
/// LanceDB calls the embedding function synchronously; this bridge uses
/// `tokio::runtime::Handle::current().block_on(...)` to run the async
/// [`EmbeddingService::embed`] call on the current Tokio runtime.
///
/// # Panics
///
/// Panics if called outside a Tokio runtime context.
pub struct AgentdEmbeddingBridge {
    service: Arc<dyn EmbeddingService>,
}

impl AgentdEmbeddingBridge {
    /// Create a new bridge wrapping `service`.
    pub fn new(service: Arc<dyn EmbeddingService>) -> Self {
        Self { service }
    }
}

impl std::fmt::Debug for AgentdEmbeddingBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentdEmbeddingBridge")
            .field("dimension", &self.service.dimension(""))
            .finish()
    }
}

impl EmbeddingFunction for AgentdEmbeddingBridge {
    fn name(&self) -> &str {
        "agentd-embedding"
    }

    fn source_type(&self) -> lancedb::Result<Cow<'_, DataType>> {
        Ok(Cow::Owned(DataType::Utf8))
    }

    fn dest_type(&self) -> lancedb::Result<Cow<'_, DataType>> {
        let dim = self.service.dimension("") as i32;
        Ok(Cow::Owned(DataType::FixedSizeList(
            Arc::new(arrow_schema::Field::new("item", DataType::Float32, true)),
            dim,
        )))
    }

    fn compute_source_embeddings(&self, source: ArrayRef) -> lancedb::Result<ArrayRef> {
        compute_embeddings(&self.service, source)
    }

    fn compute_query_embeddings(&self, input: ArrayRef) -> lancedb::Result<ArrayRef> {
        compute_embeddings(&self.service, input)
    }
}

/// Shared implementation: convert an Arrow `StringArray` into a
/// `FixedSizeListArray` of embedding vectors by calling the service.
fn compute_embeddings(
    service: &Arc<dyn EmbeddingService>,
    input: ArrayRef,
) -> lancedb::Result<ArrayRef> {
    let string_array = input.as_any().downcast_ref::<StringArray>().ok_or_else(|| {
        lancedb::Error::InvalidInput {
            message: "Expected StringArray for embedding input".to_string(),
        }
    })?;

    let texts: Vec<String> =
        (0..string_array.len()).map(|i| string_array.value(i).to_string()).collect();

    // Bridge async embed() to sync EmbeddingFunction via current Tokio handle.
    let handle = tokio::runtime::Handle::current();
    let svc = service.clone();
    let embeddings = handle
        .block_on(async move { svc.embed(&texts).await })
        .map_err(|e| lancedb::Error::Runtime { message: format!("Embedding failed: {}", e) })?;

    let dim = service.dimension("") as i32;

    let values: Vec<Option<Vec<Option<f32>>>> =
        embeddings.into_iter().map(|emb| Some(emb.into_iter().map(Some).collect())).collect();

    let array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(values, dim);
    Ok(Arc::new(array) as ArrayRef)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NoOpEmbedding;

    #[test]
    fn test_bridge_name() {
        let svc = Arc::new(NoOpEmbedding::new());
        let bridge = AgentdEmbeddingBridge::new(svc);
        assert_eq!(bridge.name(), "agentd-embedding");
    }

    #[test]
    fn test_bridge_source_type_is_utf8() {
        let svc = Arc::new(NoOpEmbedding::new());
        let bridge = AgentdEmbeddingBridge::new(svc);
        assert_eq!(bridge.source_type().unwrap().as_ref(), &DataType::Utf8);
    }

    #[test]
    fn test_bridge_debug_format() {
        let svc = Arc::new(NoOpEmbedding::new());
        let bridge = AgentdEmbeddingBridge::new(svc);
        let debug = format!("{:?}", bridge);
        assert!(debug.contains("AgentdEmbeddingBridge"));
    }

    #[test]
    fn test_bridge_dest_type_noop_is_zero_dim() {
        let svc = Arc::new(NoOpEmbedding::new());
        let bridge = AgentdEmbeddingBridge::new(svc);
        let dest = bridge.dest_type().unwrap();
        match dest.as_ref() {
            DataType::FixedSizeList(field, dim) => {
                assert_eq!(*dim, 0);
                assert_eq!(field.name(), "item");
            }
            other => panic!("Expected FixedSizeList, got {:?}", other),
        }
    }
}
