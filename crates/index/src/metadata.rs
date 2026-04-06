//! Structural metadata for code chunks.
//!
//! [`ChunkMetadata`] captures rich structural information extracted from the
//! AST during chunking — visibility modifiers, function signatures, return
//! types, and import declarations.
//!
//! # Example
//!
//! ```rust
//! use index::metadata::{ChunkMetadata, Visibility, Parameter};
//!
//! let meta = ChunkMetadata {
//!     visibility: Some(Visibility::Public),
//!     parameters: vec![
//!         Parameter { name: "x".to_string(), type_annotation: Some("i32".to_string()) },
//!     ],
//!     return_type: Some("i32".to_string()),
//!     imports: vec!["use std::sync::Arc;".to_string()],
//! };
//! assert_eq!(meta.visibility.unwrap().as_str(), "public");
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Visibility
// ---------------------------------------------------------------------------

/// Visibility modifier for a code symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Publicly exported symbol (`pub` in Rust, no `_` prefix in Python, exported in JS/TS).
    Public,
    /// Private to the containing module or file (default when no modifier is present).
    #[default]
    Private,
    /// Protected — accessible to subclasses (Python `_name` convention, TS `protected`).
    Protected,
    /// Crate-visible in Rust (`pub(crate)`).
    Crate,
    /// Module-restricted in Rust (`pub(super)` / `pub(in path)`).
    Module,
}

impl Visibility {
    /// Canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Crate => "crate",
            Visibility::Module => "module",
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Visibility {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Visibility::Public),
            "private" => Ok(Visibility::Private),
            "protected" => Ok(Visibility::Protected),
            "crate" => Ok(Visibility::Crate),
            "module" => Ok(Visibility::Module),
            _ => Err(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter
// ---------------------------------------------------------------------------

/// A single function or method parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name as written in the source.
    pub name: String,
    /// Type annotation, if present (e.g. `"i32"`, `"str"`, `"string"`).
    pub type_annotation: Option<String>,
}

// ---------------------------------------------------------------------------
// ChunkMetadata
// ---------------------------------------------------------------------------

/// Rich structural metadata extracted from the AST for a single code chunk.
///
/// Fields default to `None` / empty when the information is not applicable or
/// could not be extracted (e.g. `parameters` is empty for struct definitions).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Visibility of the top-level symbol in this chunk.
    pub visibility: Option<Visibility>,

    /// Function or method parameter list (empty for non-callable symbols).
    pub parameters: Vec<Parameter>,

    /// Return type annotation for functions and methods.
    pub return_type: Option<String>,

    /// Import/use declarations found at the top level of the source file
    /// that contains this chunk.  Provides context about available symbols.
    pub imports: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_as_str() {
        assert_eq!(Visibility::Public.as_str(), "public");
        assert_eq!(Visibility::Private.as_str(), "private");
        assert_eq!(Visibility::Protected.as_str(), "protected");
        assert_eq!(Visibility::Crate.as_str(), "crate");
        assert_eq!(Visibility::Module.as_str(), "module");
    }

    #[test]
    fn visibility_display() {
        assert_eq!(Visibility::Public.to_string(), "public");
        assert_eq!(Visibility::Private.to_string(), "private");
    }

    #[test]
    fn visibility_from_str_roundtrip() {
        use std::str::FromStr;
        for v in &[
            Visibility::Public,
            Visibility::Private,
            Visibility::Protected,
            Visibility::Crate,
            Visibility::Module,
        ] {
            let parsed = Visibility::from_str(v.as_str());
            assert_eq!(parsed, Ok(*v), "roundtrip failed for {:?}", v);
        }
    }

    #[test]
    fn visibility_from_str_unknown() {
        use std::str::FromStr;
        assert!(Visibility::from_str("banana").is_err());
    }

    #[test]
    fn visibility_default_is_private() {
        assert_eq!(Visibility::default(), Visibility::Private);
    }

    #[test]
    fn visibility_serialization() {
        let json = serde_json::to_string(&Visibility::Public).unwrap();
        assert_eq!(json, "\"public\"");
        let parsed: Visibility = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Visibility::Public);
    }

    #[test]
    fn parameter_serialization_roundtrip() {
        let p = Parameter { name: "x".to_string(), type_annotation: Some("i32".to_string()) };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: Parameter = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, p);
    }

    #[test]
    fn parameter_without_type() {
        let p = Parameter { name: "name".to_string(), type_annotation: None };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: Parameter = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.type_annotation, None);
    }

    #[test]
    fn chunk_metadata_default_is_empty() {
        let meta = ChunkMetadata::default();
        assert!(meta.visibility.is_none());
        assert!(meta.parameters.is_empty());
        assert!(meta.return_type.is_none());
        assert!(meta.imports.is_empty());
    }

    #[test]
    fn chunk_metadata_serialization_roundtrip() {
        let meta = ChunkMetadata {
            visibility: Some(Visibility::Public),
            parameters: vec![
                Parameter { name: "x".to_string(), type_annotation: Some("i32".to_string()) },
                Parameter { name: "y".to_string(), type_annotation: None },
            ],
            return_type: Some("bool".to_string()),
            imports: vec!["use std::sync::Arc;".to_string()],
        };
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: ChunkMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.visibility, meta.visibility);
        assert_eq!(parsed.parameters, meta.parameters);
        assert_eq!(parsed.return_type, meta.return_type);
        assert_eq!(parsed.imports, meta.imports);
    }
}
