//! Semantic code chunker that enriches syntactic chunks with doc comments,
//! attributes, and decorators, and applies configurable size limits.
//!
//! [`SemanticChunker`] wraps a [`SyntacticChunker`] and post-processes its
//! output to:
//!
//! - Attach preceding doc comments and attributes to their code chunk.
//! - Split chunks that exceed a configurable token budget.
//! - Produce overlapping sub-chunks for large functions so that search can
//!   retrieve them from any point in the body.
//!
//! # Example
//!
//! ```rust
//! use index::chunking::semantic::{SemanticChunker, SemanticConfig};
//! use index::chunking::types::Language;
//!
//! let config = SemanticConfig::default();
//! let chunker = SemanticChunker::new(config);
//! let source = r#"
//! /// Adds two numbers together.
//! pub fn add(a: i32, b: i32) -> i32 {
//!     a + b
//! }
//! "#;
//! let chunks = chunker.chunk("lib.rs", source, Language::Rust).unwrap();
//! // The chunk content now includes the doc comment.
//! assert!(chunks[0].content.contains("/// Adds two numbers"));
//! ```

use anyhow::Result;
use std::path::Path;

use super::syntactic::SyntacticChunker;
use super::types::{CodeChunk, Language};
use super::Chunker;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the semantic chunker.
#[derive(Debug, Clone)]
pub struct SemanticConfig {
    /// Approximate maximum number of tokens per chunk (1 token ≈ 4 bytes).
    ///
    /// Chunks that exceed this limit are split at logical line boundaries.
    pub max_tokens: usize,

    /// Number of lines to overlap between adjacent sub-chunks when a function
    /// is split due to size.
    pub overlap_lines: usize,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self { max_tokens: 2000, overlap_lines: 10 }
    }
}

impl SemanticConfig {
    /// Approximate maximum number of bytes inferred from the token limit.
    fn max_bytes(&self) -> usize {
        self.max_tokens * 4
    }
}

// ---------------------------------------------------------------------------
// SemanticChunker
// ---------------------------------------------------------------------------

/// A chunker that enriches syntactic chunks with semantic context.
///
/// Wraps [`SyntacticChunker`] and applies:
/// 1. Doc-comment / decorator prefix attachment.
/// 2. Size-bounded splitting with configurable line overlap.
pub struct SemanticChunker {
    inner: SyntacticChunker,
    config: SemanticConfig,
}

impl SemanticChunker {
    /// Create a new [`SemanticChunker`] with the given configuration.
    pub fn new(config: SemanticConfig) -> Self {
        Self { inner: SyntacticChunker::new(), config }
    }

    /// Parse `source` and return semantically enriched chunks.
    pub fn chunk(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<Vec<CodeChunk>> {
        let syntactic = self.inner.chunk(file_path, source, language)?;
        let lines: Vec<&str> = source.lines().collect();
        let enriched =
            syntactic.into_iter().flat_map(|chunk| self.enrich(chunk, &lines, language)).collect();
        Ok(enriched)
    }

    /// Detect language from file extension and chunk the file.
    pub fn chunk_path(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>> {
        let language = Language::from_path(path)
            .ok_or_else(|| anyhow::anyhow!("unsupported file extension"))?;
        let file_path = path.to_string_lossy().into_owned();
        self.chunk(&file_path, source, language)
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    /// Enrich a single syntactic chunk by:
    /// 1. Scanning backward for attached doc comments / decorators.
    /// 2. Splitting oversized chunks into overlapping sub-chunks.
    fn enrich(&self, mut chunk: CodeChunk, lines: &[&str], language: Language) -> Vec<CodeChunk> {
        // Step 1 — attach prefix (doc comments, attributes, decorators).
        attach_prefix(&mut chunk, lines, language);

        // Step 2 — split oversized chunks.
        if chunk.content.len() > self.config.max_bytes() {
            split_chunk(chunk, &self.config)
        } else {
            vec![chunk]
        }
    }
}

impl Chunker for SemanticChunker {
    fn chunk(&self, file_path: &str, source: &str, language: Language) -> Result<Vec<CodeChunk>> {
        SemanticChunker::chunk(self, file_path, source, language)
    }

    fn chunk_path(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>> {
        SemanticChunker::chunk_path(self, path, source)
    }
}

// ---------------------------------------------------------------------------
// Prefix attachment
// ---------------------------------------------------------------------------

/// Scan backward from a chunk's start line and prepend any doc comments,
/// attributes, or decorators that belong to that chunk.
///
/// The updated chunk has its `start_line` moved up and its `content`
/// prepended with the prefix lines.
fn attach_prefix(chunk: &mut CodeChunk, lines: &[&str], language: Language) {
    if chunk.start_line == 0 {
        return;
    }

    // start_line is 1-based; convert to 0-based index.
    let first_idx = chunk.start_line - 1;
    if first_idx == 0 {
        return;
    }

    let prefix_lines = collect_prefix(lines, first_idx, language);
    if prefix_lines.is_empty() {
        return;
    }

    let new_start = first_idx + 1 - prefix_lines.len(); // 1-based
    let prefix_content: String = prefix_lines.join("\n") + "\n";

    chunk.content = prefix_content + &chunk.content;
    chunk.start_line = new_start;
}

/// Walk backward from `start_idx` (exclusive upper bound, 0-based) and collect
/// contiguous doc-comment / attribute / decorator lines.
///
/// Returns lines in source order (earliest first).
fn collect_prefix(lines: &[&str], start_idx: usize, language: Language) -> Vec<String> {
    let mut collected: Vec<String> = Vec::new();

    let mut idx = start_idx; // points one past the first chunk line
    loop {
        if idx == 0 {
            break;
        }
        idx -= 1;
        let line = lines[idx].trim();

        if is_prefix_line(line, language) {
            collected.push(lines[idx].to_string());
        } else {
            break;
        }
    }

    collected.reverse(); // restore source order
    collected
}

/// Returns `true` when `line` (already trimmed) is a doc comment, attribute,
/// or decorator that should be attached to the following code element.
fn is_prefix_line(line: &str, language: Language) -> bool {
    if line.is_empty() {
        return false;
    }
    match language {
        Language::Rust => {
            line.starts_with("///")
                || line.starts_with("//!")
                || line.starts_with("/**")
                || line.starts_with("/*!")
                || line.starts_with("* ")
                || line.starts_with("*/")
                || line.starts_with("#[")
                || line.starts_with("#![")
        }
        Language::Python => {
            line.starts_with('@') || line.starts_with("\"\"\"") || line.starts_with("'''")
        }
        Language::JavaScript | Language::TypeScript => {
            line.starts_with("/**")
                || line.starts_with("* ")
                || line.starts_with("*/")
                || line.starts_with("// ")
                || line.starts_with("@")
        }
        Language::Swift => {
            // Swift doc comments use `///` or `/** */`; attributes use `@`.
            line.starts_with("///")
                || line.starts_with("/**")
                || line.starts_with("* ")
                || line.starts_with("*/")
                || line.starts_with("@")
        }
        Language::Zig => {
            // Zig doc comments use `///` (doc) or `//!` (module-level doc).
            line.starts_with("///") || line.starts_with("//!")
        }
    }
}

// ---------------------------------------------------------------------------
// Chunk splitting
// ---------------------------------------------------------------------------

/// Split an oversized chunk into overlapping sub-chunks.
///
/// Each sub-chunk covers at most `config.max_bytes()` bytes of content, with
/// `config.overlap_lines` lines of overlap between consecutive pieces.
fn split_chunk(chunk: CodeChunk, config: &SemanticConfig) -> Vec<CodeChunk> {
    let content_lines: Vec<&str> = chunk.content.lines().collect();
    let max_bytes = config.max_bytes();
    let overlap = config.overlap_lines;

    let mut result: Vec<CodeChunk> = Vec::new();
    let mut start = 0usize; // index into content_lines

    while start < content_lines.len() {
        let mut end = start;
        let mut size = 0usize;

        // Greedily include lines until we exceed max_bytes.
        while end < content_lines.len() && size + content_lines[end].len() < max_bytes {
            size += content_lines[end].len() + 1;
            end += 1;
        }
        // Ensure we always make progress.
        if end == start {
            end = start + 1;
        }

        let slice_lines = &content_lines[start..end];
        let content = slice_lines.join("\n");

        let sub_start_line = chunk.start_line + start;
        let sub_end_line = chunk.start_line + end - 1;

        result.push(CodeChunk {
            content,
            file_path: chunk.file_path.clone(),
            language: chunk.language,
            chunk_type: chunk.chunk_type,
            start_line: sub_start_line,
            end_line: sub_end_line,
            symbol_name: chunk.symbol_name.clone(),
            parent_symbol: chunk.parent_symbol.clone(),
            hierarchy_level: chunk.hierarchy_level,
            metadata: chunk.metadata.clone(),
        });

        // Move forward, leaving overlap lines for the next sub-chunk.
        if end >= content_lines.len() {
            break;
        }
        let next_start = end.saturating_sub(overlap);
        // Safety: always advance by at least one line to avoid infinite loops
        // when overlap >= (end - start).
        if next_start <= start {
            start += 1;
        } else {
            start = next_start;
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::types::ChunkType;

    fn chunker() -> SemanticChunker {
        SemanticChunker::new(SemanticConfig::default())
    }

    // ── Rust doc comments ────────────────────────────────────────────────

    #[test]
    fn rust_doc_comment_attached_to_function() {
        let source = r#"/// Returns the answer.
/// Really.
pub fn answer() -> u32 {
    42
}
"#;
        let chunks = chunker().chunk("lib.rs", source, Language::Rust).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(
            chunks[0].content.contains("/// Returns the answer."),
            "doc comment should be prepended: {:?}",
            chunks[0].content
        );
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn rust_attribute_attached_to_function() {
        let source = r#"#[inline]
#[must_use]
pub fn compute(x: i32) -> i32 {
    x * 2
}
"#;
        let chunks = chunker().chunk("lib.rs", source, Language::Rust).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("#[inline]"));
        assert!(chunks[0].content.contains("#[must_use]"));
    }

    #[test]
    fn rust_mixed_doc_and_attribute() {
        let source = r#"/// My function.
#[cfg(test)]
pub fn my_fn() {}
"#;
        let chunks = chunker().chunk("lib.rs", source, Language::Rust).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("/// My function."));
        assert!(chunks[0].content.contains("#[cfg(test)]"));
    }

    // ── Python decorators / docstrings ───────────────────────────────────

    #[test]
    fn python_decorator_attached_to_function() {
        let source = r#"@staticmethod
def helper():
    pass
"#;
        let chunks = chunker().chunk("app.py", source, Language::Python).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(
            chunks[0].content.contains("@staticmethod"),
            "decorator should be prepended: {:?}",
            chunks[0].content
        );
    }

    #[test]
    fn python_multiple_decorators() {
        let source = r#"@property
@some_decorator
def value(self):
    return self._value
"#;
        let chunks = chunker().chunk("app.py", source, Language::Python).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("@property"));
        assert!(chunks[0].content.contains("@some_decorator"));
    }

    // ── JavaScript JSDoc ─────────────────────────────────────────────────

    #[test]
    fn js_jsdoc_attached_to_function() {
        let source = r#"/**
 * Adds two numbers.
 * @param {number} a
 * @param {number} b
 * @returns {number}
 */
function add(a, b) {
    return a + b;
}
"#;
        let chunks = chunker().chunk("app.js", source, Language::JavaScript).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(
            chunks[0].content.contains("/**"),
            "JSDoc should be prepended: {:?}",
            chunks[0].content
        );
    }

    // ── No false prefix attachment ───────────────────────────────────────

    #[test]
    fn unrelated_code_not_attached() {
        let source = r#"let x = 5;

pub fn foo() -> i32 {
    x
}
"#;
        let chunks = chunker().chunk("lib.rs", source, Language::Rust).unwrap();
        let foo = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("foo")).unwrap();
        assert!(!foo.content.contains("let x = 5"), "unrelated line should not be attached");
    }

    // ── Chunk splitting ───────────────────────────────────────────────────

    #[test]
    fn large_chunk_is_split() {
        let config = SemanticConfig { max_tokens: 10, overlap_lines: 2 };
        let chunker = SemanticChunker::new(config);
        // Generate a function with many lines.
        let mut lines = vec!["pub fn big() {".to_string()];
        for i in 0..50 {
            lines.push(format!("    let _x{i} = {i};"));
        }
        lines.push("}".to_string());
        let source = lines.join("\n") + "\n";

        let chunks = chunker.chunk("lib.rs", &source, Language::Rust).unwrap();
        assert!(chunks.len() > 1, "large function should be split into multiple chunks");
    }

    #[test]
    fn small_chunk_not_split() {
        let source = "pub fn small() -> u8 { 1 }\n";
        let chunks = chunker().chunk("lib.rs", source, Language::Rust).unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn split_chunks_cover_all_lines() {
        let config = SemanticConfig { max_tokens: 5, overlap_lines: 0 };
        let chunker = SemanticChunker::new(config);
        let mut lines = vec!["pub fn big() {".to_string()];
        for i in 0..20 {
            lines.push(format!("    let _v{i} = {i};"));
        }
        lines.push("}".to_string());
        let source = lines.join("\n") + "\n";

        let chunks = chunker.chunk("lib.rs", &source, Language::Rust).unwrap();
        // All content lines must appear in at least one chunk.
        let all_content: String =
            chunks.iter().map(|c| c.content.as_str()).collect::<Vec<_>>().join(" ");
        assert!(all_content.contains("_v0"));
        assert!(all_content.contains("_v19"));
    }

    // ── Trait object ─────────────────────────────────────────────────────

    #[test]
    fn semantic_chunker_as_trait_object() {
        let chunker: Box<dyn Chunker> = Box::new(SemanticChunker::new(SemanticConfig::default()));
        let chunks =
            chunker.chunk("lib.rs", "/// Hello.\npub fn hello() {}\n", Language::Rust).unwrap();
        assert!(!chunks.is_empty());
        let c = &chunks[0];
        assert_eq!(c.chunk_type, ChunkType::Function);
        assert!(c.content.contains("/// Hello."));
    }

    #[test]
    fn chunk_path_via_semantic_chunker() {
        let chunker = SemanticChunker::new(SemanticConfig::default());
        let path = Path::new("main.py");
        let chunks = chunker.chunk_path(path, "def run(): pass\n").unwrap();
        assert_eq!(chunks.len(), 1);
    }
}
