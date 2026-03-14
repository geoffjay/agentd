//! LanceDB vector store implementation for agentd-memory.
//!
//! [`LanceStore`] implements the [`VectorStore`] trait using LanceDB — an
//! embedded vector database that requires no external server.  Data lives in a
//! local directory and is queried via Arrow-based record batches.
//!
//! # Data path
//!
//! The store opens (or creates) a LanceDB directory whose path comes from
//! [`LanceConfig`].  The default resolves to the XDG-compliant data directory
//! for `agentd-memory`:
//!
//! - **Linux**: `~/.local/share/agentd-memory/lancedb`
//! - **macOS**: `~/Library/Application Support/agentd-memory/lancedb`
//!
//! # Schema
//!
//! Each row in the `memories` table represents one [`Memory`] record plus its
//! embedding vector:
//!
//! | Column       | Arrow type                    | Notes                          |
//! |--------------|-------------------------------|--------------------------------|
//! | `id`         | `Utf8`                        | `mem_<ms>_<8hex>` format       |
//! | `content`    | `LargeUtf8`                   | Natural-language content       |
//! | `vector`     | `FixedSizeList<Float32, dim>` | Embedding vector               |
//! | `memory_type`| `Utf8`                        | `"question"` / `"information"` |
//! | `tags`       | `Utf8`                        | Comma-separated list           |
//! | `created_by` | `Utf8`                        | Actor ID                       |
//! | `created_at` | `Utf8`                        | RFC3339                        |
//! | `updated_at` | `Utf8`                        | RFC3339                        |
//! | `owner`      | `Utf8` (nullable)             | Optional owner override        |
//! | `visibility` | `Utf8`                        | `"public"` / `"private"` / …   |
//! | `shared_with`| `Utf8`                        | Comma-separated list           |
//! | `refs`       | `Utf8`                        | Comma-separated reference IDs  |
//!
//! # Search strategy
//!
//! [`LanceStore::search`] embeds the query text, performs a vector similarity
//! search with a `3×limit` over-fetch, and then post-filters results by
//! visibility, tags, and date range before returning up to `limit` items.

use std::sync::Arc;

use arrow_array::{types::Float32Type, ArrayRef, RecordBatch, RecordBatchIterator, StringArray};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use async_trait::async_trait;
use chrono::Utc;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use tracing::{debug, info, warn};

use crate::config::LanceConfig;
use crate::error::{StoreError, StoreResult};
use crate::store::{EmbeddingService, VectorStore};
use crate::types::{CreateMemoryRequest, Memory, MemoryType, SearchRequest, VisibilityLevel};

/// LanceDB vector store implementing [`VectorStore`].
pub struct LanceStore {
    db: lancedb::Connection,
    table_name: String,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl LanceStore {
    /// Open (or create) a LanceDB store at `config.path`.
    ///
    /// Only opens the connection; call [`VectorStore::initialize`] to ensure
    /// the table exists before first use.
    pub async fn new(
        config: &LanceConfig,
        embedding_service: Arc<dyn EmbeddingService>,
    ) -> StoreResult<Self> {
        let db = lancedb::connect(&config.path)
            .execute()
            .await
            .map_err(|e| StoreError::ConnectionFailed(format!("LanceDB connect failed: {}", e)))?;

        Ok(Self { db, table_name: config.table.clone(), embedding_service })
    }

    // ── Schema helpers ────────────────────────────────────────────────────

    /// Build the Arrow schema for the memories table.
    fn memory_schema(&self) -> SchemaRef {
        let dim = self.embedding_service.dimension("") as i32;
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("content", DataType::LargeUtf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                true,
            ),
            Field::new("memory_type", DataType::Utf8, false),
            Field::new("tags", DataType::Utf8, false),
            Field::new("created_by", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
            Field::new("owner", DataType::Utf8, true),
            Field::new("visibility", DataType::Utf8, false),
            Field::new("shared_with", DataType::Utf8, false),
            Field::new("refs", DataType::Utf8, false),
        ]))
    }

    /// Convert a [`Memory`] and its embedding vector into an Arrow
    /// [`RecordBatch`].
    fn memory_to_batch(&self, memory: &Memory, embedding: Vec<f32>) -> StoreResult<RecordBatch> {
        let schema = self.memory_schema();
        let dim = self.embedding_service.dimension("") as i32;

        let vector_array = arrow_array::FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            vec![Some(embedding.into_iter().map(Some).collect::<Vec<_>>())],
            dim,
        );

        let owner_array: ArrayRef = match &memory.owner {
            Some(owner) => Arc::new(StringArray::from(vec![Some(owner.as_str())])),
            None => Arc::new(StringArray::from(vec![None::<&str>])),
        };

        let memory_type_str = memory.memory_type.to_string();
        let tags_str = memory.tags.join(",");
        let created_at_str = memory.created_at.to_rfc3339();
        let updated_at_str = memory.updated_at.to_rfc3339();
        let visibility_str = memory.visibility.to_string();
        let shared_with_str = memory.shared_with.join(",");
        let refs_str = memory.references.join(",");

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec![memory.id.as_str()])),
                Arc::new(arrow_array::LargeStringArray::from(vec![memory.content.as_str()])),
                Arc::new(vector_array),
                Arc::new(StringArray::from(vec![memory_type_str.as_str()])),
                Arc::new(StringArray::from(vec![tags_str.as_str()])),
                Arc::new(StringArray::from(vec![memory.created_by.as_str()])),
                Arc::new(StringArray::from(vec![created_at_str.as_str()])),
                Arc::new(StringArray::from(vec![updated_at_str.as_str()])),
                owner_array,
                Arc::new(StringArray::from(vec![visibility_str.as_str()])),
                Arc::new(StringArray::from(vec![shared_with_str.as_str()])),
                Arc::new(StringArray::from(vec![refs_str.as_str()])),
            ],
        )
        .map_err(|e| StoreError::InvalidData(format!("Failed to create RecordBatch: {}", e)))
    }

    // ── Row conversion ────────────────────────────────────────────────────

    /// Convert a single row of a [`RecordBatch`] into a [`Memory`].
    fn batch_row_to_memory(batch: &RecordBatch, row: usize) -> StoreResult<Memory> {
        let get_str = |name: &str| -> StoreResult<String> {
            if let Some(arr) = batch.column_by_name(name) {
                if let Some(s) = arr.as_any().downcast_ref::<StringArray>() {
                    return Ok(s.value(row).to_string());
                }
                if let Some(s) = arr.as_any().downcast_ref::<arrow_array::LargeStringArray>() {
                    return Ok(s.value(row).to_string());
                }
            }
            Err(StoreError::InvalidData(format!("Missing or unreadable field: {}", name)))
        };

        let get_str_opt = |name: &str| -> Option<String> {
            if let Some(arr) = batch.column_by_name(name) {
                if arr.is_null(row) {
                    return None;
                }
                if let Some(s) = arr.as_any().downcast_ref::<StringArray>() {
                    let val = s.value(row);
                    if val.is_empty() {
                        return None;
                    }
                    return Some(val.to_string());
                }
            }
            None
        };

        let id = get_str("id")?;
        let content = get_str("content")?;

        let memory_type =
            get_str("memory_type")?.parse::<MemoryType>().map_err(StoreError::InvalidData)?;

        let tags: Vec<String> =
            get_str("tags")?.split(',').filter(|s| !s.is_empty()).map(String::from).collect();

        let created_by = get_str("created_by")?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&get_str("created_at")?)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?
            .with_timezone(&Utc);

        let updated_at = chrono::DateTime::parse_from_rfc3339(&get_str("updated_at")?)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?
            .with_timezone(&Utc);

        let owner = get_str_opt("owner");

        let visibility =
            get_str("visibility")?.parse::<VisibilityLevel>().map_err(StoreError::InvalidData)?;

        let shared_with: Vec<String> = get_str("shared_with")?
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        // `refs` column may not exist in older tables — fall back to empty vec.
        let references: Vec<String> = get_str("refs")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        Ok(Memory {
            id,
            content,
            memory_type,
            tags,
            created_by,
            created_at,
            updated_at,
            owner,
            visibility,
            shared_with,
            references,
        })
    }

    /// Drain a LanceDB result stream into a `Vec<Memory>`.
    ///
    /// Rows that fail to parse are logged and skipped rather than aborting
    /// the whole query.
    async fn collect_memories(
        &self,
        stream: lancedb::arrow::SendableRecordBatchStream,
    ) -> StoreResult<Vec<Memory>> {
        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(|e| {
            StoreError::QueryFailed(format!("Failed to collect LanceDB results: {}", e))
        })?;

        let mut memories = Vec::new();
        for batch in &batches {
            for row in 0..batch.num_rows() {
                match Self::batch_row_to_memory(batch, row) {
                    Ok(m) => memories.push(m),
                    Err(e) => warn!("Skipping unparseable memory row: {}", e),
                }
            }
        }
        Ok(memories)
    }

    /// Open the memories table, returning an error if it does not exist.
    async fn open_table(&self) -> StoreResult<lancedb::Table> {
        self.db.open_table(&self.table_name).execute().await.map_err(|e| {
            StoreError::InitializationFailed(format!(
                "Failed to open LanceDB table '{}': {}",
                self.table_name, e
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// VectorStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl VectorStore for LanceStore {
    /// Create the `memories` table with the Arrow schema derived from the
    /// embedding dimension.  A no-op if the table already exists.
    async fn initialize(&self) -> StoreResult<()> {
        let tables = self.db.table_names().execute().await.map_err(|e| {
            StoreError::InitializationFailed(format!("Failed to list LanceDB tables: {}", e))
        })?;

        if tables.contains(&self.table_name) {
            debug!("LanceDB table '{}' already exists", self.table_name);
            return Ok(());
        }

        let schema = self.memory_schema();
        self.db.create_empty_table(&self.table_name, schema).execute().await.map_err(|e| {
            StoreError::InitializationFailed(format!(
                "Failed to create LanceDB table '{}': {}",
                self.table_name, e
            ))
        })?;

        info!("Created LanceDB table '{}'", self.table_name);
        Ok(())
    }

    /// Generate an embedding for `request.content` and insert the record.
    async fn create(&self, request: CreateMemoryRequest) -> StoreResult<Memory> {
        let now = Utc::now();

        let memory = Memory {
            id: Memory::generate_id(),
            content: request.content,
            memory_type: request.memory_type,
            tags: request.tags,
            created_by: request.created_by,
            created_at: now,
            updated_at: now,
            owner: None,
            visibility: request.visibility,
            shared_with: request.shared_with,
            references: request.references,
        };

        // Embed the content
        let embeddings =
            self.embedding_service.embed(std::slice::from_ref(&memory.content)).await?;
        let embedding = embeddings.into_iter().next().ok_or_else(|| {
            StoreError::QueryFailed("Embedding service returned no vector".to_string())
        })?;

        let batch = self.memory_to_batch(&memory, embedding)?;
        let schema = batch.schema();

        let table = self.open_table().await?;
        table
            .add(RecordBatchIterator::new(vec![Ok(batch)], schema))
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to insert memory: {}", e)))?;

        Ok(memory)
    }

    /// Retrieve a single memory by exact ID match.
    async fn get(&self, id: &str) -> StoreResult<Option<Memory>> {
        let table = self.open_table().await?;

        let stream = table
            .query()
            .only_if(format!("id = '{}'", id))
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("LanceDB get query failed: {}", e)))?;

        let mut memories = self.collect_memories(stream).await?;
        Ok(memories.pop())
    }

    /// Delete a memory by ID.  Returns `false` when the ID was not found.
    async fn delete(&self, id: &str) -> StoreResult<bool> {
        if self.get(id).await?.is_none() {
            return Ok(false);
        }

        let table = self.open_table().await?;
        table
            .delete(&format!("id = '{}'", id))
            .await
            .map_err(|e| StoreError::QueryFailed(format!("LanceDB delete failed: {}", e)))?;

        Ok(true)
    }

    /// Semantic similarity search with post-filtering.
    ///
    /// Embeds `request.query`, fetches `3 × request.limit` nearest neighbours,
    /// then post-filters by visibility, tags, and creation-date range before
    /// returning up to `request.limit` results.
    async fn search(&self, request: SearchRequest) -> StoreResult<Vec<Memory>> {
        let table = self.open_table().await?;

        // Embed the query
        let embeddings = self.embedding_service.embed(std::slice::from_ref(&request.query)).await?;
        let query_vec = embeddings.into_iter().next().ok_or_else(|| {
            StoreError::QueryFailed("Embedding service returned no vector for query".to_string())
        })?;

        // Vector search — over-fetch to allow for post-filtering
        let overfetch = request.limit * 3;
        let mut builder = table
            .vector_search(query_vec)
            .map_err(|e| StoreError::QueryFailed(format!("Vector search init failed: {}", e)))?
            .limit(overfetch);

        // Optional server-side type filter
        if let Some(ref memory_type) = request.memory_type {
            builder = builder.only_if(format!("memory_type = '{}'", memory_type));
        }

        let stream = builder
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Vector search failed: {}", e)))?;

        let all = self.collect_memories(stream).await?;

        // Post-filter: visibility, tags, date range
        let filtered: Vec<Memory> = all
            .into_iter()
            .filter(|m| m.is_visible_to(request.as_actor.as_deref()))
            .filter(|m| request.tags.is_empty() || request.tags.iter().any(|t| m.tags.contains(t)))
            .filter(|m| {
                if let Some(ref from) = request.from {
                    if m.created_at < *from {
                        return false;
                    }
                }
                if let Some(ref to) = request.to {
                    if m.created_at > *to {
                        return false;
                    }
                }
                true
            })
            .take(request.limit)
            .collect();

        Ok(filtered)
    }

    /// Update the visibility tier and optional share list of a memory.
    async fn update_visibility(
        &self,
        id: &str,
        visibility: VisibilityLevel,
        shared_with: Option<Vec<String>>,
    ) -> StoreResult<Memory> {
        let mut memory = self.get(id).await?.ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        memory.visibility = visibility;
        if let Some(shared) = shared_with {
            memory.shared_with = shared;
        }
        memory.updated_at = Utc::now();

        let table = self.open_table().await?;

        // LanceDB update expressions must be SQL literals — strings need quotes.
        table
            .update()
            .only_if(format!("id = '{}'", id))
            .column("visibility", format!("'{}'", memory.visibility))
            .column("shared_with", format!("'{}'", memory.shared_with.join(",")))
            .column("updated_at", format!("'{}'", memory.updated_at.to_rfc3339()))
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to update visibility: {}", e)))?;

        Ok(memory)
    }

    /// LanceDB is embedded — a successful connection implies a healthy store.
    async fn health_check(&self) -> StoreResult<bool> {
        Ok(true)
    }

    /// Return every memory in the table (used for migration / backup).
    async fn list_all(&self) -> StoreResult<Vec<Memory>> {
        let table = self.open_table().await?;

        let stream = table
            .query()
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("list_all query failed: {}", e)))?;

        let memories = self.collect_memories(stream).await?;
        info!("list_all returned {} memories", memories.len());
        Ok(memories)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NoOpEmbedding;

    /// Build the schema for a given dimension using the same logic as
    /// [`LanceStore::memory_schema`] so we can verify field names and types
    /// without standing up a real LanceDB instance.
    fn make_schema(dim: i32) -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("content", DataType::LargeUtf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                true,
            ),
            Field::new("memory_type", DataType::Utf8, false),
            Field::new("tags", DataType::Utf8, false),
            Field::new("created_by", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
            Field::new("owner", DataType::Utf8, true),
            Field::new("visibility", DataType::Utf8, false),
            Field::new("shared_with", DataType::Utf8, false),
            Field::new("refs", DataType::Utf8, false),
        ]))
    }

    #[test]
    fn test_schema_field_count() {
        let schema = make_schema(1536);
        assert_eq!(schema.fields().len(), 12);
    }

    #[test]
    fn test_schema_field_names() {
        let schema = make_schema(1536);
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert!(names.contains(&"id"));
        assert!(names.contains(&"content"));
        assert!(names.contains(&"vector"));
        assert!(names.contains(&"memory_type"));
        assert!(names.contains(&"tags"));
        assert!(names.contains(&"created_by"));
        assert!(names.contains(&"visibility"));
        assert!(names.contains(&"shared_with"));
        assert!(names.contains(&"refs"));
        assert!(names.contains(&"owner"));
    }

    #[test]
    fn test_schema_vector_is_fixed_size_list() {
        let schema = make_schema(768);
        let vector_field = schema.field_with_name("vector").unwrap();
        match vector_field.data_type() {
            DataType::FixedSizeList(_, dim) => assert_eq!(*dim, 768),
            other => panic!("Expected FixedSizeList, got {:?}", other),
        }
    }

    #[test]
    fn test_schema_owner_is_nullable() {
        let schema = make_schema(1536);
        let owner_field = schema.field_with_name("owner").unwrap();
        assert!(owner_field.is_nullable());
    }

    #[test]
    fn test_schema_id_not_nullable() {
        let schema = make_schema(1536);
        let id_field = schema.field_with_name("id").unwrap();
        assert!(!id_field.is_nullable());
    }

    #[test]
    fn test_batch_row_to_memory_parses_correctly() {
        use arrow_array::{LargeStringArray, StringArray};
        use chrono::Utc;

        let now = Utc::now();
        let dim = 3i32;
        let schema = make_schema(dim);

        // Build a minimal record batch with one row
        let id_arr = Arc::new(StringArray::from(vec!["mem_1_abcdef01"]));
        let content_arr = Arc::new(LargeStringArray::from(vec!["Test content"]));
        let vector_arr = Arc::new(arrow_array::FixedSizeListArray::from_iter_primitive::<
            Float32Type,
            _,
            _,
        >(vec![Some(vec![Some(0.1), Some(0.2), Some(0.3)])], dim));
        let memory_type_arr = Arc::new(StringArray::from(vec!["information"]));
        let tags_arr = Arc::new(StringArray::from(vec!["tag1,tag2"]));
        let created_by_arr = Arc::new(StringArray::from(vec!["agent-1"]));
        let created_at_arr = Arc::new(StringArray::from(vec![now.to_rfc3339().as_str()]));
        let updated_at_arr = Arc::new(StringArray::from(vec![now.to_rfc3339().as_str()]));
        let owner_arr = Arc::new(StringArray::from(vec![None::<&str>]));
        let visibility_arr = Arc::new(StringArray::from(vec!["public"]));
        let shared_with_arr = Arc::new(StringArray::from(vec![""]));
        let refs_arr = Arc::new(StringArray::from(vec![""]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                id_arr,
                content_arr,
                vector_arr,
                memory_type_arr,
                tags_arr,
                created_by_arr,
                created_at_arr,
                updated_at_arr,
                owner_arr,
                visibility_arr,
                shared_with_arr,
                refs_arr,
            ],
        )
        .unwrap();

        let memory = LanceStore::batch_row_to_memory(&batch, 0).unwrap();
        assert_eq!(memory.id, "mem_1_abcdef01");
        assert_eq!(memory.content, "Test content");
        assert_eq!(memory.memory_type, MemoryType::Information);
        assert_eq!(memory.tags, vec!["tag1".to_string(), "tag2".to_string()]);
        assert_eq!(memory.created_by, "agent-1");
        assert!(memory.owner.is_none());
        assert_eq!(memory.visibility, VisibilityLevel::Public);
        assert!(memory.shared_with.is_empty());
        assert!(memory.references.is_empty());
    }

    #[test]
    fn test_batch_row_to_memory_with_references() {
        use arrow_array::{LargeStringArray, StringArray};
        use chrono::Utc;

        let now = Utc::now();
        let dim = 3i32;
        let schema = make_schema(dim);

        let id_arr = Arc::new(StringArray::from(vec!["mem_2_11111111"]));
        let content_arr = Arc::new(LargeStringArray::from(vec!["Ref content"]));
        let vector_arr = Arc::new(arrow_array::FixedSizeListArray::from_iter_primitive::<
            Float32Type,
            _,
            _,
        >(vec![Some(vec![Some(0.1), Some(0.2), Some(0.3)])], dim));
        let memory_type_arr = Arc::new(StringArray::from(vec!["information"]));
        let tags_arr = Arc::new(StringArray::from(vec![""]));
        let created_by_arr = Arc::new(StringArray::from(vec!["agent-2"]));
        let created_at_arr = Arc::new(StringArray::from(vec![now.to_rfc3339().as_str()]));
        let updated_at_arr = Arc::new(StringArray::from(vec![now.to_rfc3339().as_str()]));
        let owner_arr = Arc::new(StringArray::from(vec![Some("owner-1")]));
        let visibility_arr = Arc::new(StringArray::from(vec!["shared"]));
        let shared_with_arr = Arc::new(StringArray::from(vec!["agent-3,agent-4"]));
        let refs_arr = Arc::new(StringArray::from(vec!["mem_1_aaaaaaaa,mem_1_bbbbbbbb"]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                id_arr,
                content_arr,
                vector_arr,
                memory_type_arr,
                tags_arr,
                created_by_arr,
                created_at_arr,
                updated_at_arr,
                owner_arr,
                visibility_arr,
                shared_with_arr,
                refs_arr,
            ],
        )
        .unwrap();

        let memory = LanceStore::batch_row_to_memory(&batch, 0).unwrap();
        assert_eq!(memory.owner, Some("owner-1".to_string()));
        assert_eq!(memory.visibility, VisibilityLevel::Shared);
        assert_eq!(memory.shared_with, vec!["agent-3".to_string(), "agent-4".to_string()]);
        assert_eq!(
            memory.references,
            vec!["mem_1_aaaaaaaa".to_string(), "mem_1_bbbbbbbb".to_string()]
        );
    }

    #[test]
    fn test_noop_embedding_has_zero_dimension() {
        let svc = Arc::new(NoOpEmbedding::new());
        assert_eq!(svc.dimension(""), 0);
    }
}
