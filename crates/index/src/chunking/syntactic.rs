//! Tree-sitter based syntactic code chunker.
//!
//! [`SyntacticChunker`] parses source files using tree-sitter and extracts
//! logical code units (functions, classes, structs, etc.) as [`CodeChunk`]s.

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::{Node, Parser};

use super::types::{ChunkType, CodeChunk, Language};
use crate::metadata::{ChunkMetadata, Parameter, Visibility};

/// Tree-sitter backed chunker that splits source files into logical AST units.
pub struct SyntacticChunker;

impl SyntacticChunker {
    /// Create a new [`SyntacticChunker`].
    pub fn new() -> Self {
        Self
    }

    /// Parse `source` and extract chunks for the given `language`.
    pub fn chunk(
        &self,
        file_path: &str,
        source: &str,
        language: Language,
    ) -> Result<Vec<CodeChunk>> {
        let ts_language = ts_language_for(language);
        let mut parser = Parser::new();
        parser.set_language(&ts_language).context("failed to set tree-sitter language")?;

        let tree =
            parser.parse(source, None).context("tree-sitter failed to produce a parse tree")?;

        let source_bytes = source.as_bytes();
        let mut chunks = Vec::new();
        walk(tree.root_node(), source_bytes, file_path, language, None, None, &mut chunks);

        // Attach file-level imports to every chunk in this file.
        let imports = extract_file_imports(tree.root_node(), source_bytes, language);

        // Derive imported symbol names from the raw import strings.
        let imported_symbols: Vec<String> = imports
            .iter()
            .flat_map(|imp| crate::dependencies::extract_symbols_from_import(imp, language))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !imports.is_empty() || !imported_symbols.is_empty() {
            for chunk in &mut chunks {
                chunk.metadata.imports = imports.clone();
                chunk.metadata.imported_symbols = imported_symbols.clone();
            }
        }

        Ok(chunks)
    }

    /// Detect language from file extension and chunk the file.
    pub fn chunk_path(&self, path: &Path, source: &str) -> Result<Vec<CodeChunk>> {
        let language =
            Language::from_path(path).context("unsupported or unrecognized file extension")?;
        let file_path = path.to_string_lossy().into_owned();
        self.chunk(&file_path, source, language)
    }
}

impl Default for SyntacticChunker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Language loading
// ---------------------------------------------------------------------------

fn ts_language_for(language: Language) -> tree_sitter::Language {
    match language {
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    }
}

// ---------------------------------------------------------------------------
// Tree traversal
// ---------------------------------------------------------------------------

/// Walk a tree-sitter node recursively and collect chunks.
///
/// `parent_kind` and `parent_name` describe the nearest enclosing "container"
/// node (impl block, class definition) so that methods can be tagged with
/// their parent symbol.
fn walk(
    node: Node<'_>,
    source: &[u8],
    file_path: &str,
    language: Language,
    parent_kind: Option<&str>,
    parent_name: Option<&str>,
    chunks: &mut Vec<CodeChunk>,
) {
    let kind = node.kind();

    // Try to interpret this node as a code chunk.
    if let Some((chunk_type, symbol_name)) = classify(kind, &node, source, language, parent_kind) {
        let start_line = node.start_position().row + 1; // tree-sitter is 0-based
        let end_line = node.end_position().row + 1;
        let content = node.utf8_text(source).unwrap_or("").to_string();

        let effective_parent =
            if chunk_type == ChunkType::Method { parent_name.map(|s| s.to_string()) } else { None };

        let metadata = extract_chunk_metadata(&node, source, language, &symbol_name);

        chunks.push(CodeChunk {
            content,
            file_path: file_path.to_string(),
            language,
            chunk_type,
            start_line,
            end_line,
            symbol_name: symbol_name.clone(),
            parent_symbol: effective_parent,
            hierarchy_level: super::types::HierarchyLevel::Symbol,
            metadata,
        });

        // This node is a container; its children should know about it.
        let next_kind = Some(kind);
        let next_name = symbol_name.as_deref();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, source, file_path, language, next_kind, next_name, chunks);
        }
    } else {
        // Not a chunk node — recurse, forwarding parent context.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, source, file_path, language, parent_kind, parent_name, chunks);
        }
    }
}

/// Classify a node by kind, returning the [`ChunkType`] and extracted symbol
/// name, or `None` if this node is not a chunkable construct.
fn classify(
    kind: &str,
    node: &Node<'_>,
    source: &[u8],
    language: Language,
    parent_kind: Option<&str>,
) -> Option<(ChunkType, Option<String>)> {
    match language {
        Language::Rust => classify_rust(kind, node, source, parent_kind),
        Language::Python => classify_python(kind, node, source, parent_kind),
        Language::JavaScript => classify_js(kind, node, source),
        Language::TypeScript => classify_js(kind, node, source), // TS grammar reuses JS node kinds
    }
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn classify_rust(
    kind: &str,
    node: &Node<'_>,
    source: &[u8],
    parent_kind: Option<&str>,
) -> Option<(ChunkType, Option<String>)> {
    match kind {
        "function_item" => {
            let name = field_text(node, "name", source);
            let chunk_type = if parent_kind == Some("impl_item") {
                ChunkType::Method
            } else {
                ChunkType::Function
            };
            Some((chunk_type, name))
        }
        "struct_item" => Some((ChunkType::Struct, field_text(node, "name", source))),
        "enum_item" => Some((ChunkType::Enum, field_text(node, "name", source))),
        "trait_item" => Some((ChunkType::Trait, field_text(node, "name", source))),
        "impl_item" => {
            // Prefer trait name if implementing a trait, otherwise the type name.
            let name = field_text(node, "type", source);
            Some((ChunkType::Impl, name))
        }
        "mod_item" => Some((ChunkType::Module, field_text(node, "name", source))),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn classify_python(
    kind: &str,
    node: &Node<'_>,
    source: &[u8],
    parent_kind: Option<&str>,
) -> Option<(ChunkType, Option<String>)> {
    match kind {
        "function_definition" => {
            let name = field_text(node, "name", source);
            let chunk_type = if parent_kind == Some("class_definition") {
                ChunkType::Method
            } else {
                ChunkType::Function
            };
            Some((chunk_type, name))
        }
        "class_definition" => Some((ChunkType::Class, field_text(node, "name", source))),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// JavaScript / TypeScript
// ---------------------------------------------------------------------------

fn classify_js(kind: &str, node: &Node<'_>, source: &[u8]) -> Option<(ChunkType, Option<String>)> {
    match kind {
        "function_declaration" => Some((ChunkType::Function, field_text(node, "name", source))),
        "class_declaration" => Some((ChunkType::Class, field_text(node, "name", source))),
        "method_definition" => Some((ChunkType::Method, field_text(node, "name", source))),
        // Arrow functions / function expressions assigned to a variable.
        "lexical_declaration" | "variable_declaration" => {
            // Only chunk if the declarator directly contains an arrow_function or
            // function_expression — otherwise it's just a value assignment.
            let mut cur = node.walk();
            for child in node.children(&mut cur) {
                if child.kind() == "variable_declarator" && has_function_value(&child) {
                    let name = field_text(&child, "name", source);
                    return Some((ChunkType::Function, name));
                }
            }
            None
        }
        _ => None,
    }
}

/// Returns `true` when a `variable_declarator` node's value is a function.
fn has_function_value(node: &Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let k = child.kind();
        if k == "arrow_function" || k == "function_expression" {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the UTF-8 text of a named field child.
fn field_text(node: &Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field).and_then(|n| n.utf8_text(source).ok()).map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Metadata extraction
// ---------------------------------------------------------------------------

/// Build a [`ChunkMetadata`] for a classified AST node.
fn extract_chunk_metadata(
    node: &Node<'_>,
    source: &[u8],
    language: Language,
    symbol_name: &Option<String>,
) -> ChunkMetadata {
    ChunkMetadata {
        visibility: extract_visibility(node, source, language, symbol_name),
        parameters: extract_parameters(node, source, language),
        return_type: extract_return_type(node, source, language),
        imports: Vec::new(),          // populated by `chunk()` after the walk
        imported_symbols: Vec::new(), // populated by `chunk()` after the walk
    }
}

/// Extract the visibility modifier of a node.
fn extract_visibility(
    node: &Node<'_>,
    source: &[u8],
    language: Language,
    symbol_name: &Option<String>,
) -> Option<Visibility> {
    match language {
        Language::Rust => {
            // Look for a `visibility_modifier` child (it is not a named field in
            // tree-sitter-rust, but appears as an unnamed child).
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "visibility_modifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    return Some(if text == "pub" {
                        Visibility::Public
                    } else if text.contains("crate") {
                        Visibility::Crate
                    } else {
                        // pub(super), pub(in path), etc.
                        Visibility::Module
                    });
                }
            }
            Some(Visibility::Private)
        }
        Language::Python => {
            // Python encodes visibility by naming convention.
            let name = symbol_name.as_deref().unwrap_or("");
            if name.starts_with("__") && !name.ends_with("__") {
                Some(Visibility::Private)
            } else if name.starts_with('_') {
                Some(Visibility::Protected)
            } else {
                Some(Visibility::Public)
            }
        }
        Language::JavaScript => {
            // Plain JS has no explicit visibility keywords at the function level.
            None
        }
        Language::TypeScript => {
            // TypeScript class members may carry an `accessibility_modifier`.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "accessibility_modifier" {
                    let text = child.utf8_text(source).unwrap_or("");
                    return Some(match text {
                        "public" => Visibility::Public,
                        "protected" => Visibility::Protected,
                        "private" => Visibility::Private,
                        _ => Visibility::Public,
                    });
                }
            }
            // Top-level TS declarations are implicitly public.
            Some(Visibility::Public)
        }
    }
}

/// Extract function/method parameters from a node.
fn extract_parameters(node: &Node<'_>, source: &[u8], language: Language) -> Vec<Parameter> {
    let params_node = match language {
        Language::JavaScript | Language::TypeScript => node.child_by_field_name("parameters"),
        _ => node.child_by_field_name("parameters"),
    };

    let Some(params_node) = params_node else { return Vec::new() };

    let mut params = Vec::new();
    let mut cursor = params_node.walk();

    for child in params_node.children(&mut cursor) {
        match language {
            Language::Rust => {
                if child.kind() == "parameter" {
                    // `pattern` field: the parameter name
                    // `type` field: the type annotation
                    let name = field_text(&child, "pattern", source).unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let type_annotation = field_text(&child, "type", source);
                    params.push(Parameter { name, type_annotation });
                }
                // self_parameter, variadic_parameter: skip
            }
            Language::Python => {
                match child.kind() {
                    "identifier" => {
                        let name = child.utf8_text(source).unwrap_or("").to_string();
                        if !name.is_empty() && name != "self" && name != "cls" {
                            params.push(Parameter { name, type_annotation: None });
                        }
                    }
                    "typed_parameter" => {
                        // First named identifier child = name; subsequent = type
                        let mut inner = child.walk();
                        let mut name = String::new();
                        let mut typ: Option<String> = None;
                        for gc in child.children(&mut inner) {
                            if gc.kind() == "identifier" && name.is_empty() {
                                name = gc.utf8_text(source).unwrap_or("").to_string();
                            } else if gc.is_named() && gc.kind() != "identifier" {
                                typ = gc.utf8_text(source).ok().map(|s| s.trim().to_string());
                            }
                        }
                        if !name.is_empty() && name != "self" && name != "cls" {
                            params.push(Parameter { name, type_annotation: typ });
                        }
                    }
                    "typed_default_parameter" | "default_parameter" => {
                        let name = child
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source).ok())
                            .unwrap_or("")
                            .to_string();
                        let type_annotation = child
                            .child_by_field_name("type")
                            .and_then(|n| n.utf8_text(source).ok())
                            .map(|s| s.to_string());
                        if !name.is_empty() && name != "self" && name != "cls" {
                            params.push(Parameter { name, type_annotation });
                        }
                    }
                    _ => {}
                }
            }
            Language::JavaScript | Language::TypeScript => match child.kind() {
                "identifier" => {
                    let name = child.utf8_text(source).unwrap_or("").to_string();
                    if !name.is_empty() {
                        params.push(Parameter { name, type_annotation: None });
                    }
                }
                "required_parameter" | "optional_parameter" => {
                    let name = child
                        .child_by_field_name("pattern")
                        .and_then(|n| n.utf8_text(source).ok())
                        .unwrap_or("")
                        .to_string();
                    let type_annotation = child
                        .child_by_field_name("type")
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(|s| s.trim_start_matches(':').trim().to_string());
                    if !name.is_empty() {
                        params.push(Parameter { name, type_annotation });
                    }
                }
                "assignment_pattern" => {
                    let name = child
                        .child_by_field_name("left")
                        .and_then(|n| n.utf8_text(source).ok())
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() {
                        params.push(Parameter { name, type_annotation: None });
                    }
                }
                _ => {}
            },
        }
    }

    params
}

/// Extract the return type annotation from a function node.
fn extract_return_type(node: &Node<'_>, source: &[u8], language: Language) -> Option<String> {
    match language {
        Language::Rust => {
            // `return_type` field is the `_type` after `->` (arrow not included)
            field_text(node, "return_type", source)
        }
        Language::Python => {
            // `return_type` field is the annotation after `->`
            field_text(node, "return_type", source)
        }
        Language::JavaScript => None,
        Language::TypeScript => {
            // `return_type` is a `type_annotation` node whose text starts with `:`
            node.child_by_field_name("return_type")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.trim_start_matches(':').trim().to_string())
                .filter(|s| !s.is_empty())
        }
    }
}

/// Collect top-level import/use declarations from the file root node.
///
/// These are attached to every chunk in the file so that search results carry
/// context about what symbols the file depends on.
fn extract_file_imports(root: Node<'_>, source: &[u8], language: Language) -> Vec<String> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let is_import = match language {
            Language::Rust => child.kind() == "use_declaration",
            Language::Python => {
                matches!(child.kind(), "import_statement" | "import_from_statement")
            }
            Language::JavaScript | Language::TypeScript => child.kind() == "import_statement",
        };
        if is_import {
            if let Ok(text) = child.utf8_text(source) {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    imports.push(trimmed);
                }
            }
        }
    }
    imports
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn chunker() -> SyntacticChunker {
        SyntacticChunker::new()
    }

    // ── Rust ──────────────────────────────────────────────────────────────

    const RUST_SOURCE: &str = r#"
pub struct Foo {
    x: i32,
}

impl Foo {
    pub fn new(x: i32) -> Self {
        Foo { x }
    }

    pub fn value(&self) -> i32 {
        self.x
    }
}

pub fn standalone() -> u8 {
    42
}

pub enum Color {
    Red,
    Green,
    Blue,
}

pub trait Greet {
    fn greet(&self) -> String;
}

mod inner {
    pub fn helper() {}
}
"#;

    #[test]
    fn rust_extracts_struct() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].symbol_name.as_deref(), Some("Foo"));
    }

    #[test]
    fn rust_extracts_impl() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let impls: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Impl).collect();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].symbol_name.as_deref(), Some("Foo"));
    }

    #[test]
    fn rust_extracts_methods() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert_eq!(methods.len(), 2);
        let names: Vec<_> = methods.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "new"));
        assert!(names.iter().any(|n| n.as_str() == "value"));
    }

    #[test]
    fn rust_method_has_parent_symbol() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let method = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("new")).unwrap();
        assert_eq!(method.parent_symbol.as_deref(), Some("Foo"));
    }

    #[test]
    fn rust_extracts_standalone_function() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        assert!(fns.iter().any(|c| c.symbol_name.as_deref() == Some("standalone")));
    }

    #[test]
    fn rust_extracts_enum() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].symbol_name.as_deref(), Some("Color"));
    }

    #[test]
    fn rust_extracts_trait() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let traits: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Trait).collect();
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].symbol_name.as_deref(), Some("Greet"));
    }

    #[test]
    fn rust_extracts_module() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        let mods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Module).collect();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].symbol_name.as_deref(), Some("inner"));
    }

    #[test]
    fn rust_chunk_line_numbers_are_nonzero() {
        let chunks = chunker().chunk("foo.rs", RUST_SOURCE, Language::Rust).unwrap();
        for chunk in &chunks {
            assert!(chunk.start_line >= 1, "start_line should be 1-based");
            assert!(chunk.end_line >= chunk.start_line);
        }
    }

    // ── Python ────────────────────────────────────────────────────────────

    const PYTHON_SOURCE: &str = r#"
def greet(name: str) -> str:
    return f"Hello, {name}!"

class Animal:
    def __init__(self, name: str):
        self.name = name

    def speak(self) -> str:
        return "..."

async def fetch_data(url: str):
    pass
"#;

    #[test]
    fn python_extracts_function() {
        let chunks = chunker().chunk("app.py", PYTHON_SOURCE, Language::Python).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        let names: Vec<_> = fns.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "greet"));
        assert!(names.iter().any(|n| n.as_str() == "fetch_data"));
    }

    #[test]
    fn python_extracts_class() {
        let chunks = chunker().chunk("app.py", PYTHON_SOURCE, Language::Python).unwrap();
        let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].symbol_name.as_deref(), Some("Animal"));
    }

    #[test]
    fn python_extracts_methods() {
        let chunks = chunker().chunk("app.py", PYTHON_SOURCE, Language::Python).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert_eq!(methods.len(), 2);
        let names: Vec<_> = methods.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "__init__"));
        assert!(names.iter().any(|n| n.as_str() == "speak"));
    }

    #[test]
    fn python_method_has_parent_symbol() {
        let chunks = chunker().chunk("app.py", PYTHON_SOURCE, Language::Python).unwrap();
        let init = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("__init__")).unwrap();
        assert_eq!(init.parent_symbol.as_deref(), Some("Animal"));
    }

    // ── JavaScript ────────────────────────────────────────────────────────

    const JS_SOURCE: &str = r#"
function add(a, b) {
    return a + b;
}

class Calculator {
    constructor() {
        this.result = 0;
    }

    add(n) {
        this.result += n;
        return this;
    }
}

const multiply = (a, b) => a * b;
"#;

    #[test]
    fn js_extracts_function() {
        let chunks = chunker().chunk("app.js", JS_SOURCE, Language::JavaScript).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        assert!(fns.iter().any(|c| c.symbol_name.as_deref() == Some("add")));
    }

    #[test]
    fn js_extracts_class() {
        let chunks = chunker().chunk("app.js", JS_SOURCE, Language::JavaScript).unwrap();
        let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].symbol_name.as_deref(), Some("Calculator"));
    }

    #[test]
    fn js_extracts_methods() {
        let chunks = chunker().chunk("app.js", JS_SOURCE, Language::JavaScript).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert!(!methods.is_empty());
    }

    // ── TypeScript ────────────────────────────────────────────────────────

    const TS_SOURCE: &str = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}

class Greeter {
    constructor(private name: string) {}

    greet(): string {
        return `Hello, ${this.name}!`;
    }
}

const add = (a: number, b: number): number => a + b;
"#;

    #[test]
    fn ts_extracts_function() {
        let chunks = chunker().chunk("app.ts", TS_SOURCE, Language::TypeScript).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        assert!(fns.iter().any(|c| c.symbol_name.as_deref() == Some("greet")));
    }

    #[test]
    fn ts_extracts_class() {
        let chunks = chunker().chunk("app.ts", TS_SOURCE, Language::TypeScript).unwrap();
        let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].symbol_name.as_deref(), Some("Greeter"));
    }

    #[test]
    fn ts_extracts_methods() {
        let chunks = chunker().chunk("app.ts", TS_SOURCE, Language::TypeScript).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert!(!methods.is_empty());
    }

    // ── chunk_path ────────────────────────────────────────────────────────

    #[test]
    fn chunk_path_detects_language() {
        let path = Path::new("main.rs");
        let chunks = chunker().chunk_path(path, "pub fn foo() {}\n").unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].language, Language::Rust);
    }

    #[test]
    fn chunk_path_rejects_unknown_extension() {
        let path = Path::new("data.csv");
        assert!(chunker().chunk_path(path, "a,b,c").is_err());
    }
}
