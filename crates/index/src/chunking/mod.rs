//! Code chunking pipeline for the agentd-index service.
//!
//! This module provides the [`Chunker`] trait and two built-in implementations:
//!
//! - [`SyntacticChunker`] — tree-sitter AST-based chunker that extracts
//!   logical code units (functions, classes, structs, …).
//! - [`SemanticChunker`] — wraps [`SyntacticChunker`] and enriches chunks
//!   with doc comments, attributes/decorators, and configurable size limits.
//!
//! # Quick Start
//!
//! ```rust
//! use index::chunking::{Chunker, SyntacticChunker};
//! use index::chunking::types::Language;
//!
//! let chunker = SyntacticChunker::new();
//! let chunks = chunker.chunk("src/lib.rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }\n", Language::Rust).unwrap();
//! assert!(!chunks.is_empty());
//! ```

pub mod semantic;
pub mod syntactic;
pub mod types;

pub use semantic::{SemanticChunker, SemanticConfig};
pub use syntactic::SyntacticChunker;
pub use types::{ChunkType, CodeChunk, Language};

use anyhow::Result;
use std::path::Path;

/// The core abstraction for splitting source files into [`CodeChunk`]s.
///
/// Implementors parse source text using language-specific strategies and
/// return a flat list of chunks ordered by their position in the file.
pub trait Chunker {
    /// Parse `source` and return all extracted chunks.
    ///
    /// # Arguments
    ///
    /// * `file_path` — path string stored on each returned chunk (used for display / storage)
    /// * `source`    — full UTF-8 source text
    /// * `language`  — the programming language to use for parsing
    fn chunk(&self, file_path: &str, source: &str, language: Language) -> Result<Vec<CodeChunk>>;

    /// Detect the language from `path`'s extension and chunk the file.
    ///
    /// Returns an error if the extension is not recognized.
    fn chunk_path(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>>;
}

impl Chunker for SyntacticChunker {
    fn chunk(&self, file_path: &str, source: &str, language: Language) -> Result<Vec<CodeChunk>> {
        SyntacticChunker::chunk(self, file_path, source, language)
    }

    fn chunk_path(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>> {
        SyntacticChunker::chunk_path(self, path, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_object_works() {
        let chunker: Box<dyn Chunker> = Box::new(SyntacticChunker::new());
        let chunks = chunker
            .chunk("lib.rs", "pub fn hello() -> &'static str { \"hello\" }\n", Language::Rust)
            .unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_name.as_deref(), Some("hello"));
    }

    #[test]
    fn chunk_path_via_trait() {
        let chunker: Box<dyn Chunker> = Box::new(SyntacticChunker::new());
        let path = Path::new("main.py");
        let chunks = chunker.chunk_path(path, "def run(): pass\n").unwrap();
        assert_eq!(chunks.len(), 1);
    }
}
