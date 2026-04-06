//! BM25 keyword search strategy using an in-memory Tantivy index.
//!
//! [`KeywordIndex`] wraps a tantivy in-memory index over a set of
//! [`StoredChunk`] records.  The index is built on demand — typically from a
//! set of vector-search candidates — and queried for exact identifier matches
//! and other keyword terms.
//!
//! # Field Boosting
//!
//! `symbol_name` is indexed at a higher boost than `content` so that exact
//! matches against function names, struct names, etc. rank higher.

use std::collections::HashMap;

use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Field, Schema, SchemaBuilder, Value, FAST, STORED, TEXT},
    Index, IndexWriter, TantivyDocument,
};

use crate::store::StoredChunk;

// ---------------------------------------------------------------------------
// KeywordResult
// ---------------------------------------------------------------------------

/// A single result from a [`KeywordIndex`] query.
#[derive(Debug, Clone)]
pub struct KeywordResult {
    /// Chunk identifier matching a [`StoredChunk::id`].
    pub id: String,
    /// BM25 relevance score (higher = more relevant).
    pub score: f32,
}

// ---------------------------------------------------------------------------
// KeywordIndex
// ---------------------------------------------------------------------------

/// An in-memory tantivy index over a set of code chunks.
///
/// Build from a slice of [`StoredChunk`] references via
/// [`KeywordIndex::build`], then query with [`KeywordIndex::search`].
pub struct KeywordIndex {
    index: Index,
    id_field: Field,
    content_field: Field,
    symbol_field: Field,
}

impl KeywordIndex {
    /// Build a new in-memory keyword index over `chunks`.
    ///
    /// The index is rebuilt from scratch on every call — it is intended for
    /// small-to-medium candidate sets (e.g. the over-fetched vector results),
    /// not as a persistent store.
    pub fn build(chunks: &[&StoredChunk]) -> tantivy::Result<Self> {
        let mut schema_builder = SchemaBuilder::new();
        let id_field = schema_builder.add_text_field("id", STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let symbol_field = schema_builder.add_text_field("symbol_name", TEXT | STORED | FAST);
        let schema: Schema = schema_builder.build();

        let index = Index::create_in_ram(schema);

        let mut writer: IndexWriter = index.writer(50_000_000)?;

        for chunk in chunks {
            let symbol = chunk.chunk.symbol_name.as_deref().unwrap_or("");
            // Add both content and symbol name; symbol_name gets its own field
            // so QueryParser can boost it independently.
            let doc = doc!(
                id_field => chunk.id.as_str(),
                content_field => chunk.chunk.content.as_str(),
                symbol_field => symbol,
            );
            writer.add_document(doc)?;
        }

        writer.commit()?;

        Ok(Self { index, id_field, content_field, symbol_field })
    }

    /// Query the index and return up to `limit` results, ordered by BM25 score.
    ///
    /// Searches over `content` and `symbol_name` fields.  Symbol name matches
    /// are boosted by a factor of `3.0` via a multi-field query with a boost.
    pub fn search(&self, query_text: &str, limit: usize) -> tantivy::Result<Vec<KeywordResult>> {
        if query_text.trim().is_empty() {
            return Ok(vec![]);
        }

        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        // Build a query that searches both content and symbol_name with boost.
        let mut query_parser =
            QueryParser::for_index(&self.index, vec![self.content_field, self.symbol_field]);
        // Boost symbol_name matches (exact identifier hits rank higher).
        query_parser.set_field_boost(self.symbol_field, 3.0);
        query_parser.set_field_boost(self.content_field, 1.0);

        let query = match query_parser.parse_query(query_text) {
            Ok(q) => q,
            Err(_) => {
                // Fallback: try again with the query escaped to suppress parse errors
                // caused by special characters in identifiers (e.g. `::`, `<`, `>`).
                let escaped = escape_query(query_text);
                query_parser.parse_query(&escaped)?
            }
        };

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        // Build an id → rank map to return stable IDs.
        let mut results: Vec<KeywordResult> = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let retrieved: TantivyDocument = searcher.doc(doc_address)?;
            if let Some(id_val) = retrieved.get_first(self.id_field) {
                if let Some(id) = id_val.as_str() {
                    results.push(KeywordResult { id: id.to_string(), score });
                }
            }
        }

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape Tantivy query special characters in `s` so that arbitrary code
/// identifiers can be used as literals without causing parse errors.
fn escape_query(s: &str) -> String {
    // Tantivy special chars: + - && || ! ( ) { } [ ] ^ " ~ * ? : \ /
    s.chars()
        .flat_map(|c| {
            if matches!(
                c,
                '+' | '-'
                    | '&'
                    | '|'
                    | '!'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '^'
                    | '"'
                    | '~'
                    | '*'
                    | '?'
                    | ':'
                    | '\\'
                    | '/'
            ) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

/// Build a `HashMap<id, rank>` from a ranked result list (0 = highest rank).
pub fn rank_map(results: &[KeywordResult]) -> HashMap<String, usize> {
    results.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::types::{ChunkType, CodeChunk, HierarchyLevel, Language};
    use crate::metadata::ChunkMetadata;
    use crate::store::StoredChunk;

    fn make_chunk(id: &str, content: &str, symbol: Option<&str>) -> StoredChunk {
        StoredChunk {
            id: id.to_string(),
            repo_id: "repo1".to_string(),
            file_hash: "abc".to_string(),
            summary: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            chunk: CodeChunk {
                content: content.to_string(),
                file_path: format!("src/{id}.rs"),
                language: Language::Rust,
                chunk_type: ChunkType::Function,
                start_line: 1,
                end_line: 10,
                symbol_name: symbol.map(|s| s.to_string()),
                parent_symbol: None,
                hierarchy_level: HierarchyLevel::Symbol,
                metadata: ChunkMetadata::default(),
            },
        }
    }

    #[test]
    fn build_empty_index_succeeds() {
        let index = KeywordIndex::build(&[]).unwrap();
        let results = index.search("auth", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn empty_query_returns_empty() {
        let c = make_chunk(
            "fn_a",
            "pub fn authenticate(token: &str) -> bool { true }",
            Some("authenticate"),
        );
        let index = KeywordIndex::build(&[&c]).unwrap();
        let results = index.search("", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn finds_identifier_in_content() {
        let c1 = make_chunk(
            "fn_a",
            "pub fn authenticate(token: &str) -> bool { true }",
            Some("authenticate"),
        );
        let c2 =
            make_chunk("fn_b", "pub fn render_template(name: &str) { }", Some("render_template"));
        let index = KeywordIndex::build(&[&c1, &c2]).unwrap();
        let results = index.search("authenticate", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "fn_a");
    }

    #[test]
    fn symbol_name_gets_higher_score() {
        let c1 = make_chunk(
            "fn_auth",
            "fn process() {}",
            Some("authenticate"), // symbol matches query
        );
        let c2 = make_chunk(
            "fn_process",
            "fn authenticate_user() { /* authenticate logic */ }",
            Some("process"), // content matches but symbol doesn't
        );
        let index = KeywordIndex::build(&[&c1, &c2]).unwrap();
        let results = index.search("authenticate", 10).unwrap();
        assert!(!results.is_empty());
        // fn_auth should rank higher due to symbol_name boost
        assert_eq!(results[0].id, "fn_auth");
    }

    #[test]
    fn rank_map_produces_correct_ranks() {
        let results = vec![
            KeywordResult { id: "a".to_string(), score: 0.9 },
            KeywordResult { id: "b".to_string(), score: 0.7 },
            KeywordResult { id: "c".to_string(), score: 0.5 },
        ];
        let map = rank_map(&results);
        assert_eq!(map["a"], 0);
        assert_eq!(map["b"], 1);
        assert_eq!(map["c"], 2);
    }

    #[test]
    fn escape_query_handles_special_chars() {
        let escaped = escape_query("std::collections::HashMap");
        assert!(!escaped.contains(':') || escaped.contains("\\:"));
    }
}
