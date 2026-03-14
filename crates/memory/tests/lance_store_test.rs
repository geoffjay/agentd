//! Integration tests for the LanceDB vector store backend.
//!
//! These tests exercise the full [`VectorStore`] trait against a real LanceDB
//! instance using a temporary directory. Each test gets its own isolated
//! database to avoid interference.

use async_trait::async_trait;
use memory::config::LanceConfig;
use memory::error::{StoreError, StoreResult};
use memory::store::{EmbeddingService, LanceStore, VectorStore};
use memory::types::*;
use std::sync::Arc;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Deterministic embedding service for integration tests
// ---------------------------------------------------------------------------

/// A fake embedding service that produces deterministic vectors.
///
/// Each text is hashed into a fixed-dimension vector so that identical
/// inputs always produce identical embeddings.  This makes search results
/// predictable without requiring a real model.
struct FakeEmbedding {
    dim: usize,
}

impl FakeEmbedding {
    fn new(dim: usize) -> Self {
        Self { dim }
    }
}

#[async_trait]
impl EmbeddingService for FakeEmbedding {
    async fn embed(&self, texts: &[String]) -> StoreResult<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vec = vec![0.0f32; self.dim];
                // Simple deterministic hash: spread bytes of the text across the vector
                for (i, b) in text.bytes().enumerate() {
                    vec[i % self.dim] += b as f32 / 255.0;
                }
                // Normalize to unit length
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    vec.iter_mut().for_each(|x| *x /= norm);
                }
                vec
            })
            .collect())
    }

    fn dimension(&self, _model: &str) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn create_test_store(dir: &TempDir) -> Arc<LanceStore> {
    let config = LanceConfig {
        path: dir.path().to_string_lossy().to_string(),
        table: "test_memories".to_string(),
    };
    let embedding: Arc<dyn EmbeddingService> = Arc::new(FakeEmbedding::new(8));
    let store = LanceStore::new(&config, embedding).await.expect("LanceStore::new failed");
    store.initialize().await.expect("initialize failed");
    Arc::new(store)
}

fn make_request(content: &str, created_by: &str) -> CreateMemoryRequest {
    CreateMemoryRequest {
        content: content.to_string(),
        created_by: created_by.to_string(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Initialize
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_initialize_creates_table() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;
    // Second initialize should be a no-op (table already exists)
    store.initialize().await.expect("second initialize should succeed");
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_and_get_memory() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let req = make_request("Paris is the capital of France.", "agent-1");
    let memory = store.create(req).await.expect("create failed");

    assert!(memory.id.starts_with("mem_"));
    assert_eq!(memory.content, "Paris is the capital of France.");
    assert_eq!(memory.created_by, "agent-1");
    assert_eq!(memory.memory_type, MemoryType::Information);
    assert_eq!(memory.visibility, VisibilityLevel::Public);

    // Retrieve it
    let fetched = store.get(&memory.id).await.expect("get failed");
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, memory.id);
    assert_eq!(fetched.content, memory.content);
}

#[tokio::test]
async fn test_create_with_tags_and_references() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let req = CreateMemoryRequest {
        content: "A memory with metadata.".to_string(),
        created_by: "agent-2".to_string(),
        memory_type: MemoryType::Question,
        tags: vec!["tag-a".to_string(), "tag-b".to_string()],
        references: vec!["mem_1_ref00001".to_string()],
        visibility: VisibilityLevel::Shared,
        shared_with: vec!["agent-3".to_string()],
    };

    let memory = store.create(req).await.expect("create failed");
    assert_eq!(memory.memory_type, MemoryType::Question);
    assert_eq!(memory.tags, vec!["tag-a", "tag-b"]);
    assert_eq!(memory.references, vec!["mem_1_ref00001"]);
    assert_eq!(memory.visibility, VisibilityLevel::Shared);
    assert_eq!(memory.shared_with, vec!["agent-3"]);

    // Verify round-trip via get
    let fetched = store.get(&memory.id).await.unwrap().unwrap();
    assert_eq!(fetched.tags, vec!["tag-a", "tag-b"]);
    assert_eq!(fetched.references, vec!["mem_1_ref00001"]);
    assert_eq!(fetched.shared_with, vec!["agent-3"]);
}

// ---------------------------------------------------------------------------
// Get
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_nonexistent_returns_none() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let result = store.get("mem_0_nonexist").await.expect("get should not error");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_delete_existing_memory() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let memory = store.create(make_request("to delete", "agent-1")).await.unwrap();
    let deleted = store.delete(&memory.id).await.expect("delete failed");
    assert!(deleted);

    // Confirm it's gone
    let fetched = store.get(&memory.id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn test_delete_nonexistent_returns_false() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let deleted = store.delete("mem_0_nonexist").await.expect("delete should not error");
    assert!(!deleted);
}

// ---------------------------------------------------------------------------
// List all
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_all_returns_all_memories() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    store.create(make_request("first", "agent-1")).await.unwrap();
    store.create(make_request("second", "agent-2")).await.unwrap();
    store.create(make_request("third", "agent-1")).await.unwrap();

    let all = store.list_all().await.expect("list_all failed");
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn test_list_all_empty_table() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let all = store.list_all().await.expect("list_all failed");
    assert!(all.is_empty());
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_returns_results() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    store.create(make_request("Paris is the capital of France.", "agent-1")).await.unwrap();
    store.create(make_request("Berlin is the capital of Germany.", "agent-1")).await.unwrap();
    store.create(make_request("Rust is a programming language.", "agent-1")).await.unwrap();

    let results = store
        .search(SearchRequest {
            query: "capital of France".to_string(),
            limit: 3,
            ..Default::default()
        })
        .await
        .expect("search failed");

    // The fake embedding service produces deterministic but not semantically
    // meaningful vectors, so we just verify that vector search returns results
    // and that the content matches stored records.
    assert!(!results.is_empty(), "search should return at least one result");
    assert!(results.len() <= 3, "should respect limit");
    // All returned memories should be ones we created
    for r in &results {
        assert!(
            r.content.contains("capital") || r.content.contains("Rust"),
            "unexpected content: {}",
            r.content
        );
    }
}

#[tokio::test]
async fn test_search_respects_limit() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    for i in 0..5 {
        store
            .create(make_request(&format!("Memory number {i} about testing."), "agent-1"))
            .await
            .unwrap();
    }

    let results = store
        .search(SearchRequest { query: "testing".to_string(), limit: 2, ..Default::default() })
        .await
        .unwrap();

    assert!(results.len() <= 2);
}

// ---------------------------------------------------------------------------
// Update visibility
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_update_visibility_to_shared() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let memory = store.create(make_request("some content", "agent-1")).await.unwrap();
    assert_eq!(memory.visibility, VisibilityLevel::Public);

    let updated = store
        .update_visibility(
            &memory.id,
            VisibilityLevel::Shared,
            Some(vec!["agent-2".to_string(), "agent-3".to_string()]),
        )
        .await
        .expect("update_visibility failed");

    assert_eq!(updated.visibility, VisibilityLevel::Shared);
    assert_eq!(updated.shared_with, vec!["agent-2", "agent-3"]);

    // Verify persistence
    let fetched = store.get(&memory.id).await.unwrap().unwrap();
    assert_eq!(fetched.visibility, VisibilityLevel::Shared);
    assert_eq!(fetched.shared_with, vec!["agent-2", "agent-3"]);
}

#[tokio::test]
async fn test_update_visibility_to_private() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let memory = store.create(make_request("secret stuff", "agent-1")).await.unwrap();

    let updated =
        store.update_visibility(&memory.id, VisibilityLevel::Private, None).await.unwrap();

    assert_eq!(updated.visibility, VisibilityLevel::Private);
}

#[tokio::test]
async fn test_update_visibility_not_found() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let result = store.update_visibility("mem_0_nonexist", VisibilityLevel::Private, None).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StoreError::NotFound(_)));
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_check_returns_true() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    let healthy = store.health_check().await.expect("health_check failed");
    assert!(healthy);
}

// ---------------------------------------------------------------------------
// Visibility filtering in search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_search_filters_by_visibility() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    // Create a public memory
    store
        .create(CreateMemoryRequest {
            content: "Public knowledge about France.".to_string(),
            created_by: "agent-1".to_string(),
            visibility: VisibilityLevel::Public,
            ..Default::default()
        })
        .await
        .unwrap();

    // Create a private memory
    let private_mem = store
        .create(CreateMemoryRequest {
            content: "Private secret about France.".to_string(),
            created_by: "agent-1".to_string(),
            visibility: VisibilityLevel::Private,
            ..Default::default()
        })
        .await
        .unwrap();

    // Search as anonymous — should only see public
    let results = store
        .search(SearchRequest {
            query: "France".to_string(),
            as_actor: None,
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap();

    for r in &results {
        assert_ne!(r.id, private_mem.id, "Private memory should not be visible to anonymous");
    }
}

#[tokio::test]
async fn test_search_shared_visibility_with_actor() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    // Create a shared memory visible to agent-2
    let shared_mem = store
        .create(CreateMemoryRequest {
            content: "Shared info about testing patterns.".to_string(),
            created_by: "agent-1".to_string(),
            visibility: VisibilityLevel::Shared,
            shared_with: vec!["agent-2".to_string()],
            ..Default::default()
        })
        .await
        .unwrap();

    // Search as agent-2 (in shared_with) — should see it
    let results = store
        .search(SearchRequest {
            query: "testing patterns".to_string(),
            as_actor: Some("agent-2".to_string()),
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap();

    let found = results.iter().any(|r| r.id == shared_mem.id);
    assert!(found, "Shared memory should be visible to agent-2");

    // Search as agent-3 (NOT in shared_with) — should not see it
    let results = store
        .search(SearchRequest {
            query: "testing patterns".to_string(),
            as_actor: Some("agent-3".to_string()),
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap();

    let found = results.iter().any(|r| r.id == shared_mem.id);
    assert!(!found, "Shared memory should NOT be visible to agent-3");
}

// ---------------------------------------------------------------------------
// End-to-end: create → search → verify → delete → confirm gone
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_end_to_end_lifecycle() {
    let dir = TempDir::new().unwrap();
    let store = create_test_store(&dir).await;

    // 1. Create
    let memory = store
        .create(CreateMemoryRequest {
            content: "The Eiffel Tower is in Paris.".to_string(),
            created_by: "agent-e2e".to_string(),
            memory_type: MemoryType::Information,
            tags: vec!["landmark".to_string(), "france".to_string()],
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(memory.id.starts_with("mem_"));

    // 2. Search — should find it
    let results = store
        .search(SearchRequest {
            query: "Eiffel Tower Paris".to_string(),
            limit: 5,
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.id == memory.id));

    // 3. Update visibility
    let updated = store
        .update_visibility(
            &memory.id,
            VisibilityLevel::Shared,
            Some(vec!["agent-reader".to_string()]),
        )
        .await
        .unwrap();
    assert_eq!(updated.visibility, VisibilityLevel::Shared);

    // 4. Delete
    let deleted = store.delete(&memory.id).await.unwrap();
    assert!(deleted);

    // 5. Confirm gone
    let fetched = store.get(&memory.id).await.unwrap();
    assert!(fetched.is_none());

    let deleted_again = store.delete(&memory.id).await.unwrap();
    assert!(!deleted_again);
}
