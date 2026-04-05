//! LanceDB vector store implementation for agentd-index.
//!
//! [`LanceStore`] implements [`CodeStore`] using LanceDB — an embedded vector
//! database that requires no external server.  Data lives in a local directory
//! and is queried via Arrow-based record batches.
//!
//! # Schema
//!
//! | Column           | Arrow type                     | Notes                     |
//! |------------------|--------------------------------|---------------------------|
//! | `id`             | `Utf8`                         | `chunk_<hash>_<seq>`      |
//! | `repo_id`        | `Utf8`                         | Repository identifier     |
//! | `file_path`      | `Utf8`                         | Relative path             |
//! | `language`       | `Utf8`                         | Programming language      |
//! | `chunk_type`     | `Utf8`                         | function, class, …        |
//! | `hierarchy_level`| `Utf8`                         | symbol, file, directory … |
//! | `symbol_name`    | `Utf8` (nullable)              | Function / class name     |
//! | `parent_symbol`  | `Utf8` (nullable)              | Enclosing scope           |
//! | `start_line`     | `UInt32`                       | 1-based start             |
//! | `end_line`       | `UInt32`                       | 1-based end (inclusive)   |
//! | `content`        | `LargeUtf8`                    | Raw source text           |
//! | `summary`        | `Utf8` (nullable)              | LLM-generated summary     |
//! | `file_hash`      | `Utf8`                         | SHA-256 of source file    |
//! | `vector`         | `FixedSizeList<Float32, dim>`  | Embedding vector          |
//! | `created_at`     | `Utf8`                         | RFC3339                   |
//! | `updated_at`     | `Utf8`                         | RFC3339                   |

use std::sync::Arc;

use arrow_array::{types::Float32Type, ArrayRef, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use async_trait::async_trait;
use chrono::Utc;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use tracing::{debug, info, warn};

use crate::chunking::types::{ChunkType, CodeChunk, HierarchyLevel, Language};
use crate::config::LanceConfig;
use crate::store::error::{StoreError, StoreResult};
use crate::store::traits::{CodeStore, EmbeddingService, SearchResult, StoredChunk};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape single quotes in filter expressions.
fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Generate a deterministic chunk ID from repo, file path, and sequence index.
fn chunk_id(repo_id: &str, file_path: &str, seq: usize) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    repo_id.hash(&mut h);
    file_path.hash(&mut h);
    let hash = <std::collections::hash_map::DefaultHasher as Hasher>::finish(&h);
    format!("chunk_{:016x}_{:04}", hash, seq)
}

// ---------------------------------------------------------------------------
// LanceStore
// ---------------------------------------------------------------------------

/// LanceDB implementation of [`CodeStore`].
pub struct LanceStore {
    db: lancedb::Connection,
    table_name: String,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl LanceStore {
    /// Open (or create) a LanceDB store at `config.path`.
    ///
    /// Call [`CodeStore::initialize`] before first use to ensure the table
    /// exists.
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

    // ── Schema ────────────────────────────────────────────────────────────

    fn chunk_schema(&self) -> SchemaRef {
        let dim = self.embedding_service.dimension("") as i32;
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("repo_id", DataType::Utf8, false),
            Field::new("file_path", DataType::Utf8, false),
            Field::new("language", DataType::Utf8, false),
            Field::new("chunk_type", DataType::Utf8, false),
            Field::new("hierarchy_level", DataType::Utf8, false),
            Field::new("symbol_name", DataType::Utf8, true),
            Field::new("parent_symbol", DataType::Utf8, true),
            Field::new("start_line", DataType::UInt32, false),
            Field::new("end_line", DataType::UInt32, false),
            Field::new("content", DataType::LargeUtf8, false),
            Field::new("summary", DataType::Utf8, true),
            Field::new("file_hash", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                true,
            ),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
        ]))
    }

    // ── Batch construction ────────────────────────────────────────────────

    /// Build an Arrow [`RecordBatch`] for a slice of (id, chunk, embedding)
    /// triples, all sharing the same `repo_id`, `file_hash`, and timestamps.
    fn chunks_to_batch(
        &self,
        repo_id: &str,
        file_hash: &str,
        ids: &[String],
        chunks: &[&CodeChunk],
        embeddings: Vec<Vec<f32>>,
        now: &str,
    ) -> StoreResult<RecordBatch> {
        let schema = self.chunk_schema();
        let dim = self.embedding_service.dimension("") as i32;
        let n = ids.len();

        // Build nullable string columns.
        let make_opt_str =
            |values: Vec<Option<&str>>| -> ArrayRef { Arc::new(StringArray::from(values)) };

        let vector_array = arrow_array::FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            embeddings.into_iter().map(|v| Some(v.into_iter().map(Some).collect::<Vec<_>>())),
            dim,
        );

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(ids.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
                Arc::new(StringArray::from(vec![repo_id; n])),
                Arc::new(StringArray::from(
                    chunks.iter().map(|c| c.file_path.as_str()).collect::<Vec<_>>(),
                )),
                Arc::new(StringArray::from(
                    chunks.iter().map(|c| c.language.as_str()).collect::<Vec<_>>(),
                )),
                Arc::new(StringArray::from(
                    chunks.iter().map(|c| c.chunk_type.as_str()).collect::<Vec<_>>(),
                )),
                Arc::new(StringArray::from(
                    chunks.iter().map(|c| c.hierarchy_level.as_str()).collect::<Vec<_>>(),
                )),
                make_opt_str(chunks.iter().map(|c| c.symbol_name.as_deref()).collect()),
                make_opt_str(chunks.iter().map(|c| c.parent_symbol.as_deref()).collect()),
                Arc::new(UInt32Array::from(
                    chunks.iter().map(|c| c.start_line as u32).collect::<Vec<_>>(),
                )),
                Arc::new(UInt32Array::from(
                    chunks.iter().map(|c| c.end_line as u32).collect::<Vec<_>>(),
                )),
                Arc::new(arrow_array::LargeStringArray::from(
                    chunks.iter().map(|c| c.content.as_str()).collect::<Vec<_>>(),
                )),
                make_opt_str(vec![None; n]), // summary — filled by enrichment pipeline later
                Arc::new(StringArray::from(vec![file_hash; n])),
                Arc::new(vector_array),
                Arc::new(StringArray::from(vec![now; n])),
                Arc::new(StringArray::from(vec![now; n])),
            ],
        )
        .map_err(|e| StoreError::InvalidData(format!("Failed to create RecordBatch: {}", e)))
    }

    // ── Row conversion ────────────────────────────────────────────────────

    fn batch_row_to_stored_chunk(batch: &RecordBatch, row: usize) -> StoreResult<StoredChunk> {
        let get_str = |name: &str| -> StoreResult<String> {
            batch
                .column_by_name(name)
                .and_then(|a| {
                    a.as_any().downcast_ref::<StringArray>().map(|s| s.value(row).to_string())
                })
                .ok_or_else(|| StoreError::InvalidData(format!("Missing field: {}", name)))
        };

        let get_large_str = |name: &str| -> StoreResult<String> {
            batch
                .column_by_name(name)
                .and_then(|a| {
                    a.as_any()
                        .downcast_ref::<arrow_array::LargeStringArray>()
                        .map(|s| s.value(row).to_string())
                })
                .ok_or_else(|| StoreError::InvalidData(format!("Missing field: {}", name)))
        };

        let get_opt_str = |name: &str| -> Option<String> {
            batch.column_by_name(name).and_then(|a| {
                if a.is_null(row) {
                    return None;
                }
                a.as_any()
                    .downcast_ref::<StringArray>()
                    .map(|s| s.value(row))
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
        };

        let get_u32 = |name: &str| -> StoreResult<u32> {
            batch
                .column_by_name(name)
                .and_then(|a| a.as_any().downcast_ref::<UInt32Array>().map(|s| s.value(row)))
                .ok_or_else(|| StoreError::InvalidData(format!("Missing field: {}", name)))
        };

        let language: Language = get_str("language")?
            .parse()
            .map_err(|e: anyhow::Error| StoreError::InvalidData(e.to_string()))?;

        let chunk_type = parse_chunk_type(&get_str("chunk_type")?)?;
        let hierarchy_level = parse_hierarchy_level(&get_str("hierarchy_level")?)?;

        let chunk = CodeChunk {
            content: get_large_str("content")?,
            file_path: get_str("file_path")?,
            language,
            chunk_type,
            start_line: get_u32("start_line")? as usize,
            end_line: get_u32("end_line")? as usize,
            symbol_name: get_opt_str("symbol_name"),
            parent_symbol: get_opt_str("parent_symbol"),
            hierarchy_level,
        };

        Ok(StoredChunk {
            id: get_str("id")?,
            repo_id: get_str("repo_id")?,
            file_hash: get_str("file_hash")?,
            summary: get_opt_str("summary"),
            created_at: get_str("created_at")?,
            updated_at: get_str("updated_at")?,
            chunk,
        })
    }

    async fn collect_chunks(
        &self,
        stream: lancedb::arrow::SendableRecordBatchStream,
    ) -> StoreResult<Vec<StoredChunk>> {
        let batches: Vec<RecordBatch> = stream
            .try_collect()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to collect results: {}", e)))?;

        let mut chunks = Vec::new();
        for batch in &batches {
            for row in 0..batch.num_rows() {
                match Self::batch_row_to_stored_chunk(batch, row) {
                    Ok(c) => chunks.push(c),
                    Err(e) => warn!("Skipping unparseable chunk row: {}", e),
                }
            }
        }
        Ok(chunks)
    }

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
// CodeStore implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl CodeStore for LanceStore {
    async fn initialize(&self) -> StoreResult<()> {
        let tables = self.db.table_names().execute().await.map_err(|e| {
            StoreError::InitializationFailed(format!("Failed to list tables: {}", e))
        })?;

        if tables.contains(&self.table_name) {
            debug!("LanceDB table '{}' already exists", self.table_name);
            return Ok(());
        }

        let schema = self.chunk_schema();
        self.db.create_empty_table(&self.table_name, schema).execute().await.map_err(|e| {
            StoreError::InitializationFailed(format!(
                "Failed to create table '{}': {}",
                self.table_name, e
            ))
        })?;

        info!("Created LanceDB table '{}'", self.table_name);
        Ok(())
    }

    async fn health_check(&self) -> StoreResult<bool> {
        Ok(true)
    }

    async fn store_chunks(
        &self,
        repo_id: &str,
        file_hash: &str,
        chunks: Vec<CodeChunk>,
    ) -> StoreResult<Vec<String>> {
        if chunks.is_empty() {
            return Ok(vec![]);
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = self.embedding_service.embed(&texts).await?;

        if embeddings.len() != chunks.len() {
            return Err(StoreError::QueryFailed(format!(
                "Embedding count mismatch: got {}, expected {}",
                embeddings.len(),
                chunks.len()
            )));
        }

        let now = Utc::now().to_rfc3339();
        let ids: Vec<String> = chunks
            .iter()
            .enumerate()
            .map(|(i, _)| chunk_id(repo_id, &chunks[i].file_path, i))
            .collect();
        let chunk_refs: Vec<&CodeChunk> = chunks.iter().collect();

        let batch =
            self.chunks_to_batch(repo_id, file_hash, &ids, &chunk_refs, embeddings, &now)?;

        let table = self.open_table().await?;
        table
            .add(vec![batch])
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to insert chunks: {}", e)))?;

        debug!("Stored {} chunks for repo '{}' file '{}'", ids.len(), repo_id, chunks[0].file_path);
        Ok(ids)
    }

    async fn delete_file_chunks(&self, repo_id: &str, file_path: &str) -> StoreResult<usize> {
        // First count existing rows for this file.
        let existing = self
            .list_file_hashes(repo_id)
            .await?
            .into_iter()
            .filter(|(fp, _)| fp == file_path)
            .count();

        if existing == 0 {
            return Ok(0);
        }

        let table = self.open_table().await?;
        let safe_repo = escape_sql(repo_id);
        let safe_path = escape_sql(file_path);
        table
            .delete(&format!("repo_id = '{}' AND file_path = '{}'", safe_repo, safe_path))
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Delete failed: {}", e)))?;

        debug!("Deleted chunks for repo='{}' file='{}'", repo_id, file_path);
        // Return approximate count — LanceDB delete doesn't report rows affected.
        Ok(existing)
    }

    async fn get_file_hash(&self, repo_id: &str, file_path: &str) -> StoreResult<Option<String>> {
        let table = self.open_table().await?;
        let safe_repo = escape_sql(repo_id);
        let safe_path = escape_sql(file_path);

        let stream = table
            .query()
            .only_if(format!("repo_id = '{}' AND file_path = '{}'", safe_repo, safe_path))
            .select(lancedb::query::Select::Columns(vec!["file_hash".to_string()]))
            .limit(1)
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("get_file_hash query failed: {}", e)))?;

        let batches: Vec<RecordBatch> = stream
            .try_collect()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to collect results: {}", e)))?;

        for batch in &batches {
            if batch.num_rows() > 0 {
                if let Some(arr) = batch.column_by_name("file_hash") {
                    if let Some(s) = arr.as_any().downcast_ref::<StringArray>() {
                        return Ok(Some(s.value(0).to_string()));
                    }
                }
            }
        }
        Ok(None)
    }

    async fn list_file_hashes(&self, repo_id: &str) -> StoreResult<Vec<(String, String)>> {
        let table = self.open_table().await?;
        let safe_repo = escape_sql(repo_id);

        let stream = table
            .query()
            .only_if(format!("repo_id = '{}'", safe_repo))
            .select(lancedb::query::Select::Columns(vec![
                "file_path".to_string(),
                "file_hash".to_string(),
            ]))
            .execute()
            .await
            .map_err(|e| {
                StoreError::QueryFailed(format!("list_file_hashes query failed: {}", e))
            })?;

        let batches: Vec<RecordBatch> = stream
            .try_collect()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Failed to collect results: {}", e)))?;

        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for batch in &batches {
            let path_col = batch
                .column_by_name("file_path")
                .and_then(|a| a.as_any().downcast_ref::<StringArray>());
            let hash_col = batch
                .column_by_name("file_hash")
                .and_then(|a| a.as_any().downcast_ref::<StringArray>());

            if let (Some(paths), Some(hashes)) = (path_col, hash_col) {
                for row in 0..batch.num_rows() {
                    let path = paths.value(row).to_string();
                    let hash = hashes.value(row).to_string();
                    if seen.insert(path.clone()) {
                        result.push((path, hash));
                    }
                }
            }
        }
        Ok(result)
    }

    async fn search(
        &self,
        query: &str,
        repo_id: Option<&str>,
        limit: usize,
    ) -> StoreResult<Vec<SearchResult>> {
        let table = self.open_table().await?;

        let embeddings = self.embedding_service.embed(&[query.to_string()]).await?;
        let query_vec = embeddings.into_iter().next().ok_or_else(|| {
            StoreError::QueryFailed("Embedding service returned no vector".to_string())
        })?;

        let overfetch = limit * 3;
        let mut builder = table
            .vector_search(query_vec)
            .map_err(|e| StoreError::QueryFailed(format!("Vector search init failed: {}", e)))?
            .limit(overfetch);

        if let Some(rid) = repo_id {
            let safe = escape_sql(rid);
            builder = builder.only_if(format!("repo_id = '{}'", safe));
        }

        let stream = builder
            .execute()
            .await
            .map_err(|e| StoreError::QueryFailed(format!("Vector search failed: {}", e)))?;

        let stored = self.collect_chunks(stream).await?;
        let results = stored
            .into_iter()
            .take(limit)
            .map(|chunk| SearchResult { score: 1.0, chunk })
            .collect();

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Type parsers
// ---------------------------------------------------------------------------

fn parse_chunk_type(s: &str) -> StoreResult<ChunkType> {
    match s {
        "function" => Ok(ChunkType::Function),
        "class" => Ok(ChunkType::Class),
        "method" => Ok(ChunkType::Method),
        "struct" => Ok(ChunkType::Struct),
        "enum" => Ok(ChunkType::Enum),
        "trait" => Ok(ChunkType::Trait),
        "impl" => Ok(ChunkType::Impl),
        "module" => Ok(ChunkType::Module),
        other => Err(StoreError::InvalidData(format!("Unknown chunk_type: {}", other))),
    }
}

fn parse_hierarchy_level(s: &str) -> StoreResult<HierarchyLevel> {
    match s {
        "symbol" => Ok(HierarchyLevel::Symbol),
        "file" => Ok(HierarchyLevel::File),
        "directory" => Ok(HierarchyLevel::Directory),
        "repository" => Ok(HierarchyLevel::Repository),
        other => Err(StoreError::InvalidData(format!("Unknown hierarchy_level: {}", other))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema(dim: i32) -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("repo_id", DataType::Utf8, false),
            Field::new("file_path", DataType::Utf8, false),
            Field::new("language", DataType::Utf8, false),
            Field::new("chunk_type", DataType::Utf8, false),
            Field::new("hierarchy_level", DataType::Utf8, false),
            Field::new("symbol_name", DataType::Utf8, true),
            Field::new("parent_symbol", DataType::Utf8, true),
            Field::new("start_line", DataType::UInt32, false),
            Field::new("end_line", DataType::UInt32, false),
            Field::new("content", DataType::LargeUtf8, false),
            Field::new("summary", DataType::Utf8, true),
            Field::new("file_hash", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                true,
            ),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
        ]))
    }

    #[test]
    fn schema_field_count() {
        let schema = make_schema(768);
        assert_eq!(schema.fields().len(), 16);
    }

    #[test]
    fn schema_has_required_fields() {
        let schema = make_schema(768);
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        for required in &[
            "id",
            "repo_id",
            "file_path",
            "language",
            "chunk_type",
            "hierarchy_level",
            "content",
            "file_hash",
            "vector",
            "start_line",
            "end_line",
            "created_at",
            "updated_at",
        ] {
            assert!(names.contains(required), "missing field: {}", required);
        }
    }

    #[test]
    fn schema_vector_dim_768() {
        let schema = make_schema(768);
        let vf = schema.field_with_name("vector").unwrap();
        match vf.data_type() {
            DataType::FixedSizeList(_, dim) => assert_eq!(*dim, 768),
            other => panic!("Expected FixedSizeList, got {:?}", other),
        }
    }

    #[test]
    fn schema_nullable_fields() {
        let schema = make_schema(768);
        assert!(schema.field_with_name("symbol_name").unwrap().is_nullable());
        assert!(schema.field_with_name("parent_symbol").unwrap().is_nullable());
        assert!(schema.field_with_name("summary").unwrap().is_nullable());
    }

    #[test]
    fn schema_non_nullable_fields() {
        let schema = make_schema(768);
        assert!(!schema.field_with_name("id").unwrap().is_nullable());
        assert!(!schema.field_with_name("repo_id").unwrap().is_nullable());
        assert!(!schema.field_with_name("file_hash").unwrap().is_nullable());
    }

    #[test]
    fn chunk_id_is_deterministic() {
        let a = chunk_id("repo1", "src/lib.rs", 0);
        let b = chunk_id("repo1", "src/lib.rs", 0);
        assert_eq!(a, b);
    }

    #[test]
    fn chunk_id_differs_by_seq() {
        let a = chunk_id("repo1", "src/lib.rs", 0);
        let b = chunk_id("repo1", "src/lib.rs", 1);
        assert_ne!(a, b);
    }

    #[test]
    fn chunk_id_differs_by_file() {
        let a = chunk_id("repo1", "src/lib.rs", 0);
        let b = chunk_id("repo1", "src/main.rs", 0);
        assert_ne!(a, b);
    }

    #[test]
    fn parse_chunk_type_roundtrip() {
        for (s, expected) in &[
            ("function", ChunkType::Function),
            ("class", ChunkType::Class),
            ("method", ChunkType::Method),
            ("struct", ChunkType::Struct),
            ("enum", ChunkType::Enum),
            ("trait", ChunkType::Trait),
            ("impl", ChunkType::Impl),
            ("module", ChunkType::Module),
        ] {
            let parsed = parse_chunk_type(s).unwrap();
            assert_eq!(&parsed, expected);
        }
    }

    #[test]
    fn parse_chunk_type_unknown_errors() {
        assert!(parse_chunk_type("banana").is_err());
    }

    #[test]
    fn parse_hierarchy_level_roundtrip() {
        for (s, expected) in &[
            ("symbol", HierarchyLevel::Symbol),
            ("file", HierarchyLevel::File),
            ("directory", HierarchyLevel::Directory),
            ("repository", HierarchyLevel::Repository),
        ] {
            let parsed = parse_hierarchy_level(s).unwrap();
            assert_eq!(&parsed, expected);
        }
    }

    #[test]
    fn batch_row_to_stored_chunk_parses_correctly() {
        let dim = 3i32;
        let schema = make_schema(dim);
        let now = Utc::now().to_rfc3339();

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["chunk_abc_0000"])),
                Arc::new(StringArray::from(vec!["repo-1"])),
                Arc::new(StringArray::from(vec!["src/lib.rs"])),
                Arc::new(StringArray::from(vec!["rust"])),
                Arc::new(StringArray::from(vec!["function"])),
                Arc::new(StringArray::from(vec!["symbol"])),
                Arc::new(StringArray::from(vec![Some("my_fn")])) as ArrayRef,
                Arc::new(StringArray::from(vec![None::<&str>])) as ArrayRef,
                Arc::new(UInt32Array::from(vec![1u32])),
                Arc::new(UInt32Array::from(vec![5u32])),
                Arc::new(arrow_array::LargeStringArray::from(vec!["fn my_fn() {}"])),
                Arc::new(StringArray::from(vec![None::<&str>])) as ArrayRef,
                Arc::new(StringArray::from(vec!["deadbeef"])),
                Arc::new(
                    arrow_array::FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        vec![Some(vec![Some(0.1), Some(0.2), Some(0.3)])],
                        dim,
                    ),
                ),
                Arc::new(StringArray::from(vec![now.as_str()])),
                Arc::new(StringArray::from(vec![now.as_str()])),
            ],
        )
        .unwrap();

        let stored = LanceStore::batch_row_to_stored_chunk(&batch, 0).unwrap();
        assert_eq!(stored.id, "chunk_abc_0000");
        assert_eq!(stored.repo_id, "repo-1");
        assert_eq!(stored.file_hash, "deadbeef");
        assert_eq!(stored.chunk.file_path, "src/lib.rs");
        assert_eq!(stored.chunk.language, Language::Rust);
        assert_eq!(stored.chunk.chunk_type, ChunkType::Function);
        assert_eq!(stored.chunk.hierarchy_level, HierarchyLevel::Symbol);
        assert_eq!(stored.chunk.symbol_name, Some("my_fn".to_string()));
        assert!(stored.chunk.parent_symbol.is_none());
        assert_eq!(stored.chunk.start_line, 1);
        assert_eq!(stored.chunk.end_line, 5);
    }

    #[test]
    fn escape_sql_escapes_quotes() {
        assert_eq!(escape_sql("it's"), "it''s");
        assert_eq!(escape_sql("normal"), "normal");
    }
}
