//! Core types for code chunking.
//!
//! This module defines [`CodeChunk`], [`Language`], and [`ChunkType`] —
//! the fundamental data types produced by the chunking pipeline.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

use crate::metadata::ChunkMetadata;

/// A supported programming language for indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Swift,
}

impl Language {
    /// Detect language from file extension.
    ///
    /// Returns `None` for unrecognized extensions.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "js" | "mjs" | "cjs" => Some(Language::JavaScript),
            "ts" | "mts" | "cts" => Some(Language::TypeScript),
            "swift" => Some(Language::Swift),
            _ => None,
        }
    }

    /// Returns the canonical name string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Swift => "swift",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Ok(Language::Rust),
            "python" | "py" => Ok(Language::Python),
            "javascript" | "js" => Ok(Language::JavaScript),
            "typescript" | "ts" => Ok(Language::TypeScript),
            "swift" => Ok(Language::Swift),
            other => anyhow::bail!("Unknown language: {other}"),
        }
    }
}

/// The abstraction level of a [`CodeChunk`] in the hierarchy.
///
/// Hierarchical chunks range from coarse-grained project overviews down to
/// individual symbol-level units extracted by the syntactic/semantic chunkers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HierarchyLevel {
    /// Top-level project summary (from README, Cargo.toml/package.json, …).
    Repository,
    /// Per-directory module summary.
    Directory,
    /// Per-file summary (exports, main types, purpose).
    File,
    /// Individual symbol — function, class, struct, etc.
    #[default]
    Symbol,
}

impl HierarchyLevel {
    /// Returns the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            HierarchyLevel::Repository => "repository",
            HierarchyLevel::Directory => "directory",
            HierarchyLevel::File => "file",
            HierarchyLevel::Symbol => "symbol",
        }
    }
}

impl fmt::Display for HierarchyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The syntactic kind of a [`CodeChunk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    /// A free function or standalone function declaration.
    Function,
    /// A class definition (Python, JS/TS, Swift).
    Class,
    /// A method defined inside a class or impl block.
    Method,
    /// A struct definition (Rust, Swift).
    Struct,
    /// An enum definition (Rust, Swift).
    Enum,
    /// A trait definition (Rust).
    Trait,
    /// An impl block (Rust).
    Impl,
    /// A module declaration (Rust `mod`).
    Module,
    /// A protocol definition (Swift).
    Protocol,
    /// An extension declaration (Swift — adds methods to an existing type).
    Extension,
}

impl ChunkType {
    /// Returns the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkType::Function => "function",
            ChunkType::Class => "class",
            ChunkType::Method => "method",
            ChunkType::Struct => "struct",
            ChunkType::Enum => "enum",
            ChunkType::Trait => "trait",
            ChunkType::Impl => "impl",
            ChunkType::Module => "module",
            ChunkType::Protocol => "protocol",
            ChunkType::Extension => "extension",
        }
    }
}

impl fmt::Display for ChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single logical unit of code extracted from a source file.
///
/// Chunks correspond to top-level or nested syntactic constructs such as
/// functions, classes, structs, and impl blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// Full source text of this chunk.
    pub content: String,

    /// Absolute or relative file path the chunk came from.
    pub file_path: String,

    /// Programming language of the source file.
    pub language: Language,

    /// Syntactic kind of this chunk.
    pub chunk_type: ChunkType,

    /// One-based line number where the chunk starts.
    pub start_line: usize,

    /// One-based line number where the chunk ends (inclusive).
    pub end_line: usize,

    /// The name of the symbol (function name, struct name, etc.), if any.
    pub symbol_name: Option<String>,

    /// Name of the enclosing impl block or class, if this chunk is nested.
    pub parent_symbol: Option<String>,

    /// Abstraction level of this chunk in the index hierarchy.
    ///
    /// Defaults to [`HierarchyLevel::Symbol`] for chunks produced by the
    /// syntactic/semantic chunkers.  Hierarchical chunks (file, directory,
    /// repository) are produced by [`crate::chunking::hierarchical::HierarchicalChunker`].
    #[serde(default)]
    pub hierarchy_level: HierarchyLevel,

    /// Rich structural metadata extracted from the AST (visibility, parameters,
    /// return type, and file-level imports).  Defaults to an empty [`ChunkMetadata`]
    /// for hierarchy-level chunks that are not backed by a parsed symbol.
    #[serde(default)]
    pub metadata: ChunkMetadata,
}

impl CodeChunk {
    /// Number of lines spanned by this chunk.
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_language_from_path_rs() {
        assert_eq!(Language::from_path(Path::new("foo.rs")), Some(Language::Rust));
    }

    #[test]
    fn test_language_from_path_py() {
        assert_eq!(Language::from_path(Path::new("bar.py")), Some(Language::Python));
    }

    #[test]
    fn test_language_from_path_js() {
        assert_eq!(Language::from_path(Path::new("app.js")), Some(Language::JavaScript));
        assert_eq!(Language::from_path(Path::new("app.mjs")), Some(Language::JavaScript));
    }

    #[test]
    fn test_language_from_path_ts() {
        assert_eq!(Language::from_path(Path::new("main.ts")), Some(Language::TypeScript));
    }

    #[test]
    fn test_language_from_path_swift() {
        assert_eq!(Language::from_path(Path::new("App.swift")), Some(Language::Swift));
    }

    #[test]
    fn test_language_from_path_unknown() {
        assert_eq!(Language::from_path(Path::new("readme.md")), None);
        assert_eq!(Language::from_path(Path::new("noext")), None);
    }

    #[test]
    fn test_language_as_str() {
        assert_eq!(Language::Rust.as_str(), "rust");
        assert_eq!(Language::Python.as_str(), "python");
        assert_eq!(Language::JavaScript.as_str(), "javascript");
        assert_eq!(Language::TypeScript.as_str(), "typescript");
        assert_eq!(Language::Swift.as_str(), "swift");
    }

    #[test]
    fn test_language_from_str() {
        assert_eq!("rust".parse::<Language>().unwrap(), Language::Rust);
        assert_eq!("py".parse::<Language>().unwrap(), Language::Python);
        assert_eq!("js".parse::<Language>().unwrap(), Language::JavaScript);
        assert_eq!("ts".parse::<Language>().unwrap(), Language::TypeScript);
        assert_eq!("swift".parse::<Language>().unwrap(), Language::Swift);
        assert!("cobol".parse::<Language>().is_err());
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
    }

    #[test]
    fn test_chunk_type_as_str() {
        assert_eq!(ChunkType::Function.as_str(), "function");
        assert_eq!(ChunkType::Class.as_str(), "class");
        assert_eq!(ChunkType::Method.as_str(), "method");
        assert_eq!(ChunkType::Struct.as_str(), "struct");
        assert_eq!(ChunkType::Enum.as_str(), "enum");
        assert_eq!(ChunkType::Trait.as_str(), "trait");
        assert_eq!(ChunkType::Impl.as_str(), "impl");
        assert_eq!(ChunkType::Module.as_str(), "module");
        assert_eq!(ChunkType::Protocol.as_str(), "protocol");
        assert_eq!(ChunkType::Extension.as_str(), "extension");
    }

    #[test]
    fn test_chunk_line_count() {
        let chunk = CodeChunk {
            content: "fn foo() {}".to_string(),
            file_path: "src/lib.rs".to_string(),
            language: Language::Rust,
            chunk_type: ChunkType::Function,
            start_line: 5,
            end_line: 10,
            symbol_name: Some("foo".to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::Symbol,
            metadata: Default::default(),
        };
        assert_eq!(chunk.line_count(), 6);
    }

    #[test]
    fn test_chunk_serialization() {
        let chunk = CodeChunk {
            content: "def hello(): pass".to_string(),
            file_path: "main.py".to_string(),
            language: Language::Python,
            chunk_type: ChunkType::Function,
            start_line: 1,
            end_line: 1,
            symbol_name: Some("hello".to_string()),
            parent_symbol: None,
            hierarchy_level: HierarchyLevel::Symbol,
            metadata: Default::default(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let parsed: CodeChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.symbol_name, chunk.symbol_name);
        assert_eq!(parsed.language, chunk.language);
    }

    #[test]
    fn test_language_from_path_uses_pathbuf() {
        let path = PathBuf::from("/home/user/project/src/main.rs");
        assert_eq!(Language::from_path(&path), Some(Language::Rust));
    }
}
