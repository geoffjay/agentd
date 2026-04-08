//! Dependency mapping and import relationship analysis.
//!
//! [`DependencyGraph`] tracks file-level import relationships so that agents
//! can navigate "who imports this file" and "what does this file import".
//!
//! # Example
//!
//! ```rust
//! use index::dependencies::{DependencyGraph, FileDependencies};
//!
//! let deps = vec![
//!     FileDependencies {
//!         file_path: "src/a.rs".to_string(),
//!         imported_files: vec!["src/b.rs".to_string()],
//!         imported_symbols: vec!["Foo".to_string()],
//!     },
//!     FileDependencies {
//!         file_path: "src/b.rs".to_string(),
//!         imported_files: vec![],
//!         imported_symbols: vec![],
//!     },
//! ];
//! let graph = DependencyGraph::build(&deps);
//! assert_eq!(graph.get_importees("src/a.rs"), &["src/b.rs"]);
//! assert_eq!(graph.get_importers("src/b.rs"), &["src/a.rs"]);
//! ```

use std::collections::HashMap;

use crate::chunking::types::Language;
use crate::store::traits::SearchResult;

// ---------------------------------------------------------------------------
// FileDependencies
// ---------------------------------------------------------------------------

/// Import relationship data for a single source file.
#[derive(Debug, Clone, Default)]
pub struct FileDependencies {
    /// Path to the source file.
    pub file_path: String,
    /// Resolved or partial file paths that this file imports.
    ///
    /// For Rust `use` statements these are module paths (`"foo::bar"`).
    /// For Python/JS/TS these are the module specifiers as written.
    pub imported_files: Vec<String>,
    /// Specific symbol names imported by this file (e.g. `Foo`, `bar`, `MyTrait`).
    pub imported_symbols: Vec<String>,
}

// ---------------------------------------------------------------------------
// DependencyGraph
// ---------------------------------------------------------------------------

/// A graph of file-level import relationships built from [`FileDependencies`].
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// file -> files that this file imports (its dependencies / importees).
    importees: HashMap<String, Vec<String>>,
    /// file -> files that import this file (its dependents / importers).
    importers: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Build a [`DependencyGraph`] from a slice of per-file dependency information.
    pub fn build(deps: &[FileDependencies]) -> Self {
        let mut graph = DependencyGraph::default();
        for dep in deps {
            let importees = dep.imported_files.clone();
            for importee in &importees {
                graph.importers.entry(importee.clone()).or_default().push(dep.file_path.clone());
            }
            graph.importees.entry(dep.file_path.clone()).or_default().extend(importees);
        }
        graph
    }

    /// Return the files that `file` imports (its direct dependencies).
    pub fn get_importees(&self, file: &str) -> &[String] {
        self.importees.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return the files that import `file` (its direct dependents).
    pub fn get_importers(&self, file: &str) -> &[String] {
        self.importers.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Return all files directly related to `file` (importers + importees combined).
    pub fn related_files(&self, file: &str) -> Vec<String> {
        let mut related = Vec::new();
        related.extend_from_slice(self.get_importees(file));
        for imp in self.get_importers(file) {
            if !related.contains(imp) {
                related.push(imp.clone());
            }
        }
        related
    }

    /// Return `true` when the graph contains no entries.
    pub fn is_empty(&self) -> bool {
        self.importees.is_empty() && self.importers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Import symbol extraction
// ---------------------------------------------------------------------------

/// Extract imported symbol names from a raw import statement string.
///
/// Uses simple pattern matching on the import text rather than re-running the
/// tree-sitter parser, since the text is already available from [`ChunkMetadata::imports`].
///
/// Returns an empty list for wildcard imports or statements that cannot be parsed.
pub fn extract_symbols_from_import(import_text: &str, language: Language) -> Vec<String> {
    let text = import_text.trim();
    match language {
        Language::Rust => extract_rust_symbols(text),
        Language::Python => extract_python_symbols(text),
        Language::JavaScript | Language::TypeScript => extract_js_symbols(text),
        Language::Swift => extract_swift_symbols(text),
        Language::Zig => extract_zig_symbols(text),
        Language::Go => extract_go_symbols(text),
        Language::Ruby => extract_ruby_symbols(text),
    }
}

/// Extract symbols from a Rust `use` statement.
///
/// Examples:
/// - `use foo::Bar;` -> `["Bar"]`
/// - `use foo::{Bar, Baz};` -> `["Bar", "Baz"]`
/// - `use foo::bar::*;` -> `[]` (wildcard)
/// - `extern crate foo;` -> `["foo"]`
/// - `mod foo;` -> `["foo"]`
fn extract_rust_symbols(text: &str) -> Vec<String> {
    if text.starts_with("extern crate ") {
        let name =
            text.trim_start_matches("extern crate ").trim_end_matches(';').trim().to_string();
        return if name.is_empty() { vec![] } else { vec![name] };
    }
    if text.starts_with("mod ") && text.ends_with(';') {
        let name = text.trim_start_matches("mod ").trim_end_matches(';').trim().to_string();
        return if name.is_empty() { vec![] } else { vec![name] };
    }
    if !text.starts_with("use ") {
        return vec![];
    }
    let body = text.trim_start_matches("use ").trim_end_matches(';');
    // Wildcard import
    if body.ends_with("::*") || body == "*" {
        return vec![];
    }
    // Braced group: use foo::{Bar, Baz}
    if let Some(brace_start) = body.rfind('{') {
        if let Some(brace_end) = body.rfind('}') {
            let inner = &body[brace_start + 1..brace_end];
            return inner
                .split(',')
                .map(|s| s.trim().split(" as ").next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty() && s != "*")
                .collect();
        }
    }
    // Single symbol: use foo::Bar or use foo::Bar as Alias
    let last = body.rsplit("::").next().unwrap_or(body);
    let name = last.split(" as ").next().unwrap_or("").trim().to_string();
    if name.is_empty() || name == "*" {
        vec![]
    } else {
        vec![name]
    }
}

/// Extract symbols from a Python import statement.
///
/// Examples:
/// - `import foo` -> `["foo"]`
/// - `from foo import bar, baz` -> `["bar", "baz"]`
/// - `from foo import *` -> `[]`
fn extract_python_symbols(text: &str) -> Vec<String> {
    if text.starts_with("from ") {
        // `from foo import bar, baz` or `from foo import (bar, baz)`
        if let Some(import_pos) = text.find(" import ") {
            let after = text[import_pos + 8..].trim().trim_matches(|c| c == '(' || c == ')');
            if after == "*" {
                return vec![];
            }
            return after
                .split(',')
                .map(|s| s.trim().split(" as ").next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    } else if text.starts_with("import ") {
        // `import foo` or `import foo as f, bar as b`
        return text
            .trim_start_matches("import ")
            .split(',')
            .map(|s| s.trim().split(" as ").next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    vec![]
}

/// Extract symbols from a JavaScript / TypeScript import statement.
///
/// Examples:
/// - `import { foo, bar } from './module'` -> `["foo", "bar"]`
/// - `import foo from './module'` -> `["foo"]`
/// - `import * as ns from './module'` -> `[]` (namespace)
/// - `const foo = require('./module')` -> `["foo"]`
fn extract_js_symbols(text: &str) -> Vec<String> {
    // Named imports: import { foo, bar } from '...'
    if let Some(brace_start) = text.find('{') {
        if let Some(brace_end) = text.find('}') {
            let inner = &text[brace_start + 1..brace_end];
            return inner
                .split(',')
                .map(|s| s.trim().split(" as ").next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    // Namespace import: import * as ns -- return empty (wildcard-like)
    if text.contains("* as ") {
        return vec![];
    }
    // Default import: import foo from '...'
    if text.starts_with("import ") {
        let after = text.trim_start_matches("import ").trim();
        if let Some(from_pos) = after.find(" from ") {
            let name = after[..from_pos].trim().to_string();
            if !name.is_empty() && !name.starts_with('*') {
                return vec![name];
            }
        }
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Dependency-aware search boost
// ---------------------------------------------------------------------------

/// Extract imported symbol names from a Swift `import` declaration.
///
/// Swift import forms:
/// - `import Foundation`           → `["Foundation"]`
/// - `import UIKit.UIView`         → `["UIKit"]` (top-level module)
/// - `import class Foundation.NSDate` → `["NSDate"]` (specific declaration)
fn extract_swift_symbols(text: &str) -> Vec<String> {
    // Strip optional `import` keyword prefix.
    let after = match text.strip_prefix("import") {
        Some(rest) => rest.trim(),
        None => return vec![],
    };
    if after.is_empty() {
        return vec![];
    }

    // Handle `import <kind> Module.Symbol` (e.g. `import class Foundation.NSDate`).
    // Swift import kinds: class, struct, enum, protocol, typealias, func, var, let
    let swift_kinds = ["class", "struct", "enum", "protocol", "typealias", "func", "var", "let"];
    let module_path = if let Some(kind) = swift_kinds.iter().find(|&&k| after.starts_with(k)) {
        after[kind.len()..].trim()
    } else {
        after
    };

    // Return the last dotted component as the imported symbol name.
    // `Foundation.NSDate` → `["NSDate"]`, `Foundation` → `["Foundation"]`.
    let symbol = module_path.split('.').next_back().unwrap_or(module_path).trim().to_string();
    if symbol.is_empty() {
        vec![]
    } else {
        vec![symbol]
    }
}

/// Extract imported symbol names from a Zig `const x = @import("…")` statement.
///
/// Examples:
/// - `const std = @import("std");`               → `["std"]`
/// - `const fs = @import("std").fs;`             → `["std"]`
/// - `const Allocator = @import("mem.zig");`     → `["mem"]`
fn extract_zig_symbols(text: &str) -> Vec<String> {
    // Find `@import("…")` and extract the module path string.
    if let Some(start) = text.find("@import(\"") {
        let after = &text[start + 9..]; // skip `@import("`
        if let Some(end) = after.find('"') {
            let module_path = &after[..end];
            // Use the stem of the path (last component without extension).
            let stem = module_path
                .rsplit('/')
                .next()
                .unwrap_or(module_path)
                .trim_end_matches(".zig")
                .trim_end_matches(".o");
            if !stem.is_empty() {
                return vec![stem.to_string()];
            }
        }
    }
    vec![]
}

/// Extract symbols from a Go import declaration.
///
/// Go imports are either a single quoted path or a grouped block:
/// - `import "fmt"` → `["fmt"]`
/// - `import "github.com/foo/bar"` → `["bar"]`
/// - `import (\n\t"fmt"\n\t"os"\n)` → `["fmt", "os"]`
/// - `import alias "pkg/path"` → `["alias"]`
fn extract_go_symbols(text: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    // Strip leading `import` keyword and surrounding whitespace.
    let body = text.trim().trim_start_matches("import").trim();

    // Collect all quoted strings from the import text.
    let mut rest = body;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('"') {
            let path = &after[..end];
            // The package name is the last path segment, without the version suffix.
            let segment = path.rsplit('/').next().unwrap_or(path);
            // Strip major-version suffixes like `/v2`
            let name = segment.split('.').next().unwrap_or(segment);
            if !name.is_empty() {
                symbols.push(name.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }

    // Respect explicit aliases: `alias "path"` — the alias should replace the path stem.
    // Simple heuristic: if a word precedes the first quote on its line, treat it as alias.
    // We rebuild: scan lines for `<ident> "<path>"` patterns.
    let mut aliased: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("//")
            || line.starts_with("import")
            || line.starts_with('(')
            || line.starts_with(')')
        {
            continue;
        }
        // Check for `alias "path"` or just `"path"`
        if let Some(q) = line.find('"') {
            let before = line[..q].trim();
            if !before.is_empty()
                && before != "_"
                && before.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                aliased.push(before.to_string());
            }
        }
    }

    // If we found aliased imports, replace the last N symbols with the aliases.
    // (simple: just return aliased when the counts match, else return path-stems).
    if !aliased.is_empty() && aliased.len() == symbols.len() {
        return aliased;
    }

    symbols
}

/// Extract symbols from a Ruby `require` or `require_relative` call.
///
/// Examples:
/// - `require 'ostruct'`          → `["ostruct"]`
/// - `require_relative './base'`  → `["base"]`
/// - `require "net/http"`         → `["http"]`
fn extract_ruby_symbols(text: &str) -> Vec<String> {
    let text = text.trim();
    // Strip the method name prefix.
    let after = if let Some(rest) = text.strip_prefix("require_relative") {
        rest
    } else if let Some(rest) = text.strip_prefix("require") {
        rest
    } else {
        return vec![];
    };
    let after = after.trim().trim_start_matches('(').trim_end_matches(')');
    // Extract the string literal contents (single or double quoted).
    let inner = after.trim().trim_start_matches(['"', '\'']).trim_end_matches(['"', '\'']);
    if inner.is_empty() {
        return vec![];
    }
    // Use last path segment as the symbol name (strip leading `./`, `../`).
    let segment = inner.rsplit('/').next().unwrap_or(inner);
    // Strip file extension if present.
    let name = segment.split('.').next().unwrap_or(segment).trim_matches('_');
    if name.is_empty() {
        vec![]
    } else {
        vec![name.to_string()]
    }
}

/// Boost search result scores for files related to the top results via imports.
///
/// For each result in the top `top_n` results, files that import it or are
/// imported by it receive a score boost of `boost_factor`.  Results are
/// re-sorted by descending score after boosting.
pub fn boost_results_by_dependencies(
    mut results: Vec<SearchResult>,
    graph: &DependencyGraph,
    top_n: usize,
    boost_factor: f32,
) -> Vec<SearchResult> {
    if graph.is_empty() || results.is_empty() {
        return results;
    }

    // Collect file paths of the top results.
    let top_files: std::collections::HashSet<String> =
        results.iter().take(top_n).map(|r| r.chunk.chunk.file_path.clone()).collect();

    // Collect related files.
    let mut boost_files: std::collections::HashSet<String> = std::collections::HashSet::new();
    for file in &top_files {
        for related in graph.related_files(file) {
            if !top_files.contains(&related) {
                boost_files.insert(related);
            }
        }
    }

    // Apply boost.
    for result in &mut results {
        if boost_files.contains(&result.chunk.chunk.file_path) {
            result.score += boost_factor;
        }
    }

    // Re-sort descending.
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- extract_rust_symbols -----------------------------------------------

    #[test]
    fn rust_use_single_symbol() {
        let syms = extract_symbols_from_import("use std::sync::Arc;", Language::Rust);
        assert_eq!(syms, vec!["Arc"]);
    }

    #[test]
    fn rust_use_braced_symbols() {
        let syms = extract_symbols_from_import("use std::sync::{Arc, Mutex};", Language::Rust);
        assert!(syms.contains(&"Arc".to_string()));
        assert!(syms.contains(&"Mutex".to_string()));
    }

    #[test]
    fn rust_use_wildcard_returns_empty() {
        let syms = extract_symbols_from_import("use foo::*;", Language::Rust);
        assert!(syms.is_empty());
    }

    #[test]
    fn rust_extern_crate() {
        let syms = extract_symbols_from_import("extern crate serde;", Language::Rust);
        assert_eq!(syms, vec!["serde"]);
    }

    #[test]
    fn rust_mod_declaration() {
        let syms = extract_symbols_from_import("mod utils;", Language::Rust);
        assert_eq!(syms, vec!["utils"]);
    }

    #[test]
    fn rust_use_with_alias() {
        let syms = extract_symbols_from_import("use foo::Bar as MyBar;", Language::Rust);
        assert_eq!(syms, vec!["Bar"]);
    }

    // -- extract_python_symbols ---------------------------------------------

    #[test]
    fn python_from_import() {
        let syms =
            extract_symbols_from_import("from os.path import join, exists", Language::Python);
        assert!(syms.contains(&"join".to_string()));
        assert!(syms.contains(&"exists".to_string()));
    }

    #[test]
    fn python_import_module() {
        let syms = extract_symbols_from_import("import os", Language::Python);
        assert_eq!(syms, vec!["os"]);
    }

    #[test]
    fn python_import_wildcard_returns_empty() {
        let syms = extract_symbols_from_import("from foo import *", Language::Python);
        assert!(syms.is_empty());
    }

    #[test]
    fn python_from_import_with_alias() {
        let syms =
            extract_symbols_from_import("from numpy import array as np_array", Language::Python);
        assert_eq!(syms, vec!["array"]);
    }

    // -- extract_js_symbols ------------------------------------------------

    #[test]
    fn js_named_imports() {
        let syms = extract_symbols_from_import(
            "import { foo, bar } from './module'",
            Language::JavaScript,
        );
        assert!(syms.contains(&"foo".to_string()));
        assert!(syms.contains(&"bar".to_string()));
    }

    #[test]
    fn js_default_import() {
        let syms = extract_symbols_from_import("import React from 'react'", Language::JavaScript);
        assert_eq!(syms, vec!["React"]);
    }

    #[test]
    fn ts_named_imports() {
        let syms = extract_symbols_from_import(
            "import { Component, OnInit } from '@angular/core'",
            Language::TypeScript,
        );
        assert!(syms.contains(&"Component".to_string()));
        assert!(syms.contains(&"OnInit".to_string()));
    }

    #[test]
    fn js_namespace_import_returns_empty() {
        let syms =
            extract_symbols_from_import("import * as utils from './utils'", Language::JavaScript);
        assert!(syms.is_empty());
    }

    // -- extract_swift_symbols ---------------------------------------------

    #[test]
    fn swift_simple_import() {
        let syms = extract_symbols_from_import("import Foundation", Language::Swift);
        assert_eq!(syms, vec!["Foundation"]);
    }

    #[test]
    fn swift_import_with_submodule() {
        let syms = extract_symbols_from_import("import UIKit.UIView", Language::Swift);
        assert_eq!(syms, vec!["UIView"]);
    }

    #[test]
    fn swift_import_specific_declaration() {
        let syms = extract_symbols_from_import("import class Foundation.NSDate", Language::Swift);
        assert_eq!(syms, vec!["NSDate"]);
    }

    #[test]
    fn swift_import_struct_kind() {
        let syms = extract_symbols_from_import("import struct Swift.Array", Language::Swift);
        assert_eq!(syms, vec!["Array"]);
    }

    // -- extract_zig_symbols -----------------------------------------------

    #[test]
    fn zig_std_import() {
        let syms = extract_symbols_from_import(r#"const std = @import("std");"#, Language::Zig);
        assert_eq!(syms, vec!["std"]);
    }

    #[test]
    fn zig_file_import() {
        let syms =
            extract_symbols_from_import(r#"const utils = @import("utils.zig");"#, Language::Zig);
        assert_eq!(syms, vec!["utils"]);
    }

    #[test]
    fn zig_nested_path_import() {
        let syms =
            extract_symbols_from_import(r#"const foo = @import("sub/foo.zig");"#, Language::Zig);
        assert_eq!(syms, vec!["foo"]);
    }

    #[test]
    fn zig_non_import_returns_empty() {
        let syms = extract_symbols_from_import("const x: i32 = 42;", Language::Zig);
        assert!(syms.is_empty());
    }

    // -- extract_go_symbols ------------------------------------------------

    #[test]
    fn go_single_import() {
        let syms = extract_symbols_from_import("import \"fmt\"", Language::Go);
        assert_eq!(syms, vec!["fmt"]);
    }

    #[test]
    fn go_grouped_imports() {
        let text = "import (\n\t\"fmt\"\n\t\"strings\"\n)";
        let syms = extract_symbols_from_import(text, Language::Go);
        assert!(syms.contains(&"fmt".to_string()), "should include fmt");
        assert!(syms.contains(&"strings".to_string()), "should include strings");
    }

    #[test]
    fn go_long_path_uses_last_segment() {
        let syms = extract_symbols_from_import("import \"github.com/foo/bar\"", Language::Go);
        assert_eq!(syms, vec!["bar"]);
    }

    #[test]
    fn go_aliased_import() {
        let text = "import (\n\tmyfmt \"fmt\"\n)";
        let syms = extract_symbols_from_import(text, Language::Go);
        assert_eq!(syms, vec!["myfmt"]);
    }

    // -- extract_ruby_symbols ----------------------------------------------

    #[test]
    fn ruby_require_simple() {
        let syms = extract_symbols_from_import("require 'ostruct'", Language::Ruby);
        assert_eq!(syms, vec!["ostruct"]);
    }

    #[test]
    fn ruby_require_double_quotes() {
        let syms = extract_symbols_from_import("require \"net/http\"", Language::Ruby);
        assert_eq!(syms, vec!["http"]);
    }

    #[test]
    fn ruby_require_relative() {
        let syms = extract_symbols_from_import("require_relative './base'", Language::Ruby);
        assert_eq!(syms, vec!["base"]);
    }

    #[test]
    fn ruby_require_relative_parent() {
        let syms = extract_symbols_from_import("require_relative '../models/user'", Language::Ruby);
        assert_eq!(syms, vec!["user"]);
    }

    // -- DependencyGraph ---------------------------------------------------

    fn make_deps() -> Vec<FileDependencies> {
        vec![
            FileDependencies {
                file_path: "src/a.rs".to_string(),
                imported_files: vec!["src/b.rs".to_string(), "src/c.rs".to_string()],
                imported_symbols: vec!["Foo".to_string()],
            },
            FileDependencies {
                file_path: "src/b.rs".to_string(),
                imported_files: vec!["src/c.rs".to_string()],
                imported_symbols: vec![],
            },
            FileDependencies {
                file_path: "src/c.rs".to_string(),
                imported_files: vec![],
                imported_symbols: vec![],
            },
        ]
    }

    #[test]
    fn graph_get_importees() {
        let graph = DependencyGraph::build(&make_deps());
        let importees = graph.get_importees("src/a.rs");
        assert!(importees.contains(&"src/b.rs".to_string()));
        assert!(importees.contains(&"src/c.rs".to_string()));
    }

    #[test]
    fn graph_get_importers() {
        let graph = DependencyGraph::build(&make_deps());
        let importers = graph.get_importers("src/c.rs");
        assert!(importers.contains(&"src/a.rs".to_string()));
        assert!(importers.contains(&"src/b.rs".to_string()));
    }

    #[test]
    fn graph_get_importees_empty_for_leaf() {
        let graph = DependencyGraph::build(&make_deps());
        assert!(graph.get_importees("src/c.rs").is_empty());
    }

    #[test]
    fn graph_get_importers_empty_for_root() {
        let graph = DependencyGraph::build(&make_deps());
        assert!(graph.get_importers("src/a.rs").is_empty());
    }

    #[test]
    fn graph_related_files_combines_both() {
        let graph = DependencyGraph::build(&make_deps());
        let related = graph.related_files("src/b.rs");
        // b imports c, and a imports b
        assert!(related.contains(&"src/c.rs".to_string()));
        assert!(related.contains(&"src/a.rs".to_string()));
    }

    #[test]
    fn graph_is_empty_for_default() {
        let graph = DependencyGraph::default();
        assert!(graph.is_empty());
    }

    #[test]
    fn graph_unknown_file_returns_empty() {
        let graph = DependencyGraph::build(&make_deps());
        assert!(graph.get_importees("src/nonexistent.rs").is_empty());
        assert!(graph.get_importers("src/nonexistent.rs").is_empty());
    }
}
