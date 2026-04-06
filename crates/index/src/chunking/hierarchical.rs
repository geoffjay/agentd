//! Hierarchical indexing — multi-level code summaries.
//!
//! [`HierarchicalChunker`] takes a flat list of [`CodeChunk`]s (from the
//! syntactic or semantic chunker) and produces *additional* summary chunks at
//! three coarser levels of abstraction:
//!
//! | Level        | Content                                              |
//! |--------------|------------------------------------------------------|
//! | File         | Aggregates all symbols in one source file            |
//! | Directory    | Aggregates all file summaries in a directory         |
//! | Repository   | Top-level overview from all directory summaries      |
//!
//! Symbol-level chunks are passed through unchanged; the three new levels
//! are appended after them.
//!
//! # Example
//!
//! ```rust
//! use index::chunking::hierarchical::{HierarchicalChunker, HierarchicalConfig};
//! use index::chunking::types::HierarchyLevel;
//! use index::chunking::{SyntacticChunker, Chunker, Language};
//!
//! let syntactic = SyntacticChunker::new();
//! let symbol_chunks = syntactic
//!     .chunk("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }\n", Language::Rust)
//!     .unwrap();
//!
//! let hier = HierarchicalChunker::new(HierarchicalConfig::default());
//! let all_chunks = hier.build(symbol_chunks);
//!
//! assert!(all_chunks.iter().any(|c| c.hierarchy_level == HierarchyLevel::File));
//! assert!(all_chunks.iter().any(|c| c.hierarchy_level == HierarchyLevel::Directory));
//! assert!(all_chunks.iter().any(|c| c.hierarchy_level == HierarchyLevel::Repository));
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::{ChunkType, CodeChunk, HierarchyLevel, Language};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the hierarchical chunker.
#[derive(Debug, Clone, Default)]
pub struct HierarchicalConfig {
    /// Repository name to use in the top-level summary.
    ///
    /// If `None`, a generic "Repository" label is used.
    pub repo_name: Option<String>,

    /// Optional project description to embed in the repository-level chunk.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// HierarchicalChunker
// ---------------------------------------------------------------------------

/// Produces multi-level index entries from a flat list of symbol chunks.
///
/// Call [`HierarchicalChunker::build`] with the output of a syntactic or
/// semantic chunker to obtain the enriched, hierarchical chunk list.
pub struct HierarchicalChunker {
    config: HierarchicalConfig,
}

impl HierarchicalChunker {
    /// Create a new [`HierarchicalChunker`] with the given configuration.
    pub fn new(config: HierarchicalConfig) -> Self {
        Self { config }
    }

    /// Build a hierarchical index from `symbol_chunks`.
    ///
    /// Returns the original `symbol_chunks` with file-, directory-, and
    /// repository-level chunks appended.
    pub fn build(&self, symbol_chunks: Vec<CodeChunk>) -> Vec<CodeChunk> {
        let file_chunks = self.build_file_chunks(&symbol_chunks);
        let dir_chunks = self.build_dir_chunks(&file_chunks);
        let repo_chunk = self.build_repo_chunk(&dir_chunks);

        let mut result = symbol_chunks;
        result.extend(file_chunks);
        result.extend(dir_chunks);
        result.push(repo_chunk);
        result
    }

    // -----------------------------------------------------------------------
    // File-level summaries
    // -----------------------------------------------------------------------

    fn build_file_chunks(&self, symbol_chunks: &[CodeChunk]) -> Vec<CodeChunk> {
        // Group symbol chunks by file path.
        let mut by_file: HashMap<String, Vec<&CodeChunk>> = HashMap::new();
        for chunk in symbol_chunks {
            by_file.entry(chunk.file_path.clone()).or_default().push(chunk);
        }

        let mut file_chunks: Vec<CodeChunk> =
            by_file.iter().map(|(file_path, chunks)| self.file_chunk(file_path, chunks)).collect();

        // Sort by file path for deterministic output.
        file_chunks.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        file_chunks
    }

    fn file_chunk(&self, file_path: &str, chunks: &[&CodeChunk]) -> CodeChunk {
        let language = chunks.first().map(|c| c.language).unwrap_or(Language::Rust);

        // Collect export list (unique, sorted symbol names).
        let mut symbols: Vec<String> = chunks
            .iter()
            .filter_map(|c| c.symbol_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        symbols.sort();

        // Count by chunk type.
        let type_counts = count_by_type(chunks);

        let content = build_file_summary(file_path, language, &symbols, &type_counts);

        // Line span: from first to last symbol in the file.
        let start_line = chunks.iter().map(|c| c.start_line).min().unwrap_or(1);
        let end_line = chunks.iter().map(|c| c.end_line).max().unwrap_or(start_line);

        CodeChunk {
            content,
            file_path: file_path.to_string(),
            language,
            chunk_type: ChunkType::Module,
            start_line,
            end_line,
            symbol_name: Some(file_path.to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::File,
            metadata: Default::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Directory-level summaries
    // -----------------------------------------------------------------------

    fn build_dir_chunks(&self, file_chunks: &[CodeChunk]) -> Vec<CodeChunk> {
        // Group file chunks by their parent directory.
        let mut by_dir: HashMap<String, Vec<&CodeChunk>> = HashMap::new();
        for chunk in file_chunks {
            let dir = parent_dir(&chunk.file_path);
            by_dir.entry(dir).or_default().push(chunk);
        }

        let mut dir_chunks: Vec<CodeChunk> =
            by_dir.iter().map(|(dir, chunks)| self.dir_chunk(dir, chunks)).collect();

        dir_chunks.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        dir_chunks
    }

    fn dir_chunk(&self, dir: &str, file_chunks: &[&CodeChunk]) -> CodeChunk {
        // Infer a representative language (most common among file chunks).
        let language = most_common_language(file_chunks);

        let file_names: Vec<String> = {
            let mut v: Vec<String> = file_chunks
                .iter()
                .map(|c| {
                    Path::new(&c.file_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| c.file_path.clone())
                })
                .collect();
            v.sort();
            v.dedup();
            v
        };

        let content = build_dir_summary(dir, &file_names, file_chunks.len());

        CodeChunk {
            content,
            file_path: dir.to_string(),
            language,
            chunk_type: ChunkType::Module,
            start_line: 1,
            end_line: 1,
            symbol_name: Some(dir.to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::Directory,
            metadata: Default::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Repository-level summary
    // -----------------------------------------------------------------------

    fn build_repo_chunk(&self, dir_chunks: &[CodeChunk]) -> CodeChunk {
        let repo_name = self.config.repo_name.as_deref().unwrap_or("Repository");

        let dirs: Vec<String> = {
            let mut v: Vec<String> =
                dir_chunks.iter().filter_map(|c| c.symbol_name.clone()).collect();
            v.sort();
            v.dedup();
            v
        };

        let total_files: usize = dir_chunks.len();
        let dir_refs: Vec<&CodeChunk> = dir_chunks.iter().collect();
        let language = most_common_language(&dir_refs);

        let content =
            build_repo_summary(repo_name, self.config.description.as_deref(), &dirs, total_files);

        CodeChunk {
            content,
            file_path: ".".to_string(),
            language,
            chunk_type: ChunkType::Module,
            start_line: 1,
            end_line: 1,
            symbol_name: Some(repo_name.to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::Repository,
            metadata: Default::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the parent directory of `file_path` as a string.
fn parent_dir(file_path: &str) -> String {
    PathBuf::from(file_path)
        .parent()
        .map(|p| if p.as_os_str().is_empty() { Path::new(".") } else { p })
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string())
}

/// Count [`CodeChunk`]s by their [`ChunkType`].
fn count_by_type(chunks: &[&CodeChunk]) -> HashMap<ChunkType, usize> {
    let mut map: HashMap<ChunkType, usize> = HashMap::new();
    for chunk in chunks {
        *map.entry(chunk.chunk_type).or_insert(0) += 1;
    }
    map
}

/// Return the most common [`Language`] in a slice of chunks.
fn most_common_language(chunks: &[&CodeChunk]) -> Language {
    let mut counts: HashMap<Language, usize> = HashMap::new();
    for c in chunks {
        *counts.entry(c.language).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(lang, _)| lang).unwrap_or(Language::Rust)
}

// ---------------------------------------------------------------------------
// Content builders
// ---------------------------------------------------------------------------

fn build_file_summary(
    file_path: &str,
    language: Language,
    symbols: &[String],
    type_counts: &HashMap<ChunkType, usize>,
) -> String {
    let mut lines = vec![format!("# File: {file_path}"), format!("Language: {language}")];

    // Type breakdown.
    let mut type_summary: Vec<String> =
        type_counts.iter().map(|(t, n)| format!("{} {}", n, t.as_str())).collect();
    type_summary.sort();
    if !type_summary.is_empty() {
        lines.push(format!("Contains: {}", type_summary.join(", ")));
    }

    // Exports.
    if !symbols.is_empty() {
        lines.push(format!("Symbols: {}", symbols.join(", ")));
    }

    lines.join("\n")
}

fn build_dir_summary(dir: &str, file_names: &[String], file_count: usize) -> String {
    let mut lines = vec![format!("# Directory: {dir}"), format!("Files: {file_count}")];
    if !file_names.is_empty() {
        lines.push(format!("Contained files: {}", file_names.join(", ")));
    }
    lines.join("\n")
}

fn build_repo_summary(
    repo_name: &str,
    description: Option<&str>,
    dirs: &[String],
    total_dirs: usize,
) -> String {
    let mut lines = vec![format!("# Repository: {repo_name}")];
    if let Some(desc) = description {
        lines.push(format!("Description: {desc}"));
    }
    lines.push(format!("Directories: {total_dirs}"));
    if !dirs.is_empty() {
        lines.push(format!("Modules: {}", dirs.join(", ")));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::{Language, SyntacticChunker};

    fn sample_chunks() -> Vec<CodeChunk> {
        let chunker = SyntacticChunker::new();
        let rust_src = "pub fn add(a: i32, b: i32) -> i32 { a + b }\npub fn sub(a: i32, b: i32) -> i32 { a - b }\n";
        let mut chunks = chunker.chunk("src/math.rs", rust_src, Language::Rust).unwrap();

        let py_src = "def greet(name): return f'Hello, {name}!'\n";
        chunks.extend(chunker.chunk("src/greet.py", py_src, Language::Python).unwrap());

        chunks
    }

    #[test]
    fn build_produces_file_chunks() {
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(chunks);

        let file_chunks: Vec<_> =
            all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::File).collect();
        assert_eq!(file_chunks.len(), 2, "one file chunk per source file");
    }

    #[test]
    fn build_produces_directory_chunk() {
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(chunks);

        let dir_chunks: Vec<_> =
            all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::Directory).collect();
        assert!(!dir_chunks.is_empty(), "at least one directory chunk expected");
    }

    #[test]
    fn build_produces_repository_chunk() {
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(chunks);

        let repo_chunks: Vec<_> =
            all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::Repository).collect();
        assert_eq!(repo_chunks.len(), 1, "exactly one repository-level chunk");
    }

    #[test]
    fn symbol_chunks_preserved() {
        let input = sample_chunks();
        let input_len = input.len();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(input);

        let symbol_count =
            all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::Symbol).count();
        assert_eq!(symbol_count, input_len, "all input symbol chunks must be preserved");
    }

    #[test]
    fn file_chunk_contains_symbol_names() {
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(chunks);

        let math_file = all
            .iter()
            .find(|c| c.hierarchy_level == HierarchyLevel::File && c.file_path.contains("math"))
            .unwrap();
        assert!(math_file.content.contains("add"), "file summary should list 'add'");
        assert!(math_file.content.contains("sub"), "file summary should list 'sub'");
    }

    #[test]
    fn dir_chunk_contains_file_names() {
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(chunks);

        let dir_chunk =
            all.iter().find(|c| c.hierarchy_level == HierarchyLevel::Directory).unwrap();
        assert!(
            dir_chunk.content.contains("math.rs") || dir_chunk.content.contains("greet.py"),
            "directory summary should name its files: {:?}",
            dir_chunk.content
        );
    }

    #[test]
    fn repo_chunk_includes_repo_name() {
        let config = HierarchicalConfig {
            repo_name: Some("my-project".to_string()),
            description: Some("A test project".to_string()),
        };
        let chunks = sample_chunks();
        let hier = HierarchicalChunker::new(config);
        let all = hier.build(chunks);

        let repo = all.iter().find(|c| c.hierarchy_level == HierarchyLevel::Repository).unwrap();
        assert!(repo.content.contains("my-project"));
        assert!(repo.content.contains("A test project"));
    }

    #[test]
    fn hierarchy_level_display() {
        assert_eq!(HierarchyLevel::Repository.to_string(), "repository");
        assert_eq!(HierarchyLevel::Directory.to_string(), "directory");
        assert_eq!(HierarchyLevel::File.to_string(), "file");
        assert_eq!(HierarchyLevel::Symbol.to_string(), "symbol");
    }

    #[test]
    fn hierarchy_level_default_is_symbol() {
        assert_eq!(HierarchyLevel::default(), HierarchyLevel::Symbol);
    }

    #[test]
    fn hierarchy_level_serialization() {
        let level = HierarchyLevel::File;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"file\"");
        let parsed: HierarchyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, HierarchyLevel::File);
    }

    #[test]
    fn empty_input_produces_single_repo_chunk() {
        let hier = HierarchicalChunker::new(HierarchicalConfig::default());
        let all = hier.build(vec![]);

        // Should still produce file/dir/repo structure (just empty).
        assert_eq!(all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::File).count(), 0);
        assert_eq!(
            all.iter().filter(|c| c.hierarchy_level == HierarchyLevel::Repository).count(),
            1
        );
    }
}
