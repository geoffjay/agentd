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
        Language::Swift => tree_sitter_swift::LANGUAGE.into(),
        Language::Zig => tree_sitter_zig::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
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
        Language::Swift => classify_swift(kind, node, source, parent_kind),
        Language::Zig => classify_zig(kind, node, source, parent_kind),
        Language::Go => classify_go(kind, node, source),
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
// Swift
// ---------------------------------------------------------------------------

/// Classify a Swift AST node.
///
/// In tree-sitter-swift the grammar uses `class_declaration` for **all** named
/// type declarations (class, struct, enum, extension).  The actual kind is
/// determined by the first non-named keyword child (`class`, `struct`, `enum`,
/// `extension`).
///
/// Constructs mapped to chunk types:
/// - `function_declaration` / `protocol_function_declaration` → [`ChunkType::Function`] or [`ChunkType::Method`]
/// - `init_declaration` → [`ChunkType::Method`]
/// - `class_declaration` with keyword `class` → [`ChunkType::Class`]
/// - `class_declaration` with keyword `struct` → [`ChunkType::Struct`]
/// - `class_declaration` with keyword `enum` → [`ChunkType::Enum`]
/// - `class_declaration` with keyword `extension` → [`ChunkType::Extension`]
/// - `protocol_declaration` → [`ChunkType::Protocol`]
fn classify_swift(
    kind: &str,
    node: &Node<'_>,
    source: &[u8],
    parent_kind: Option<&str>,
) -> Option<(ChunkType, Option<String>)> {
    /// Container node kinds whose children are treated as methods.
    const METHOD_PARENTS: &[&str] = &["class_declaration", "protocol_declaration"];

    match kind {
        "function_declaration" | "protocol_function_declaration" => {
            let name = field_text(node, "name", source)
                .or_else(|| swift_first_simple_identifier(node, source));
            let chunk_type = if parent_kind.is_some_and(|k| METHOD_PARENTS.contains(&k)) {
                ChunkType::Method
            } else {
                ChunkType::Function
            };
            Some((chunk_type, name))
        }
        "init_declaration" => {
            // Initializers are always methods inside a type body.
            Some((ChunkType::Method, Some("init".to_string())))
        }
        "class_declaration" => {
            // This grammar node covers class / struct / enum / extension.
            // Inspect the first keyword child to determine the real kind.
            let keyword = swift_type_keyword(node, source);
            let name = field_text(node, "name", source)
                .or_else(|| swift_first_simple_identifier(node, source));
            let chunk_type = match keyword.as_deref() {
                Some("struct") => ChunkType::Struct,
                Some("enum") => ChunkType::Enum,
                Some("extension") => ChunkType::Extension,
                _ => ChunkType::Class, // "class" or unrecognised
            };
            Some((chunk_type, name))
        }
        "protocol_declaration" => {
            let name = field_text(node, "name", source)
                .or_else(|| swift_first_simple_identifier(node, source));
            Some((ChunkType::Protocol, name))
        }
        _ => None,
    }
}

/// Identify the Swift type keyword (`class`, `struct`, `enum`, `extension`)
/// from a `class_declaration` node by scanning its direct unnamed children.
fn swift_type_keyword(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source) {
                let s = text.trim();
                if matches!(s, "class" | "struct" | "enum" | "extension") {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

/// Return the text of the first `simple_identifier` direct child of `node`.
fn swift_first_simple_identifier(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "simple_identifier" {
            if let Ok(text) = child.utf8_text(source) {
                let s = text.trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

/// Classify a Go AST node.
///
/// Go source constructs mapped to chunk types:
/// - `function_declaration` → [`ChunkType::Function`]
/// - `method_declaration`   → [`ChunkType::Method`]
/// - `type_spec` with `struct_type` body → [`ChunkType::Struct`]
/// - `type_spec` with `interface_type` body → [`ChunkType::Trait`]
fn classify_go(kind: &str, node: &Node<'_>, source: &[u8]) -> Option<(ChunkType, Option<String>)> {
    match kind {
        "function_declaration" => Some((ChunkType::Function, field_text(node, "name", source))),
        "method_declaration" => Some((ChunkType::Method, field_text(node, "name", source))),
        "type_spec" => {
            let name = field_text(node, "name", source);
            let type_node = node.child_by_field_name("type")?;
            let chunk_type = match type_node.kind() {
                "struct_type" => ChunkType::Struct,
                "interface_type" => ChunkType::Trait,
                _ => return None,
            };
            Some((chunk_type, name))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Zig
// ---------------------------------------------------------------------------

/// Classify a Zig AST node.
///
/// Zig source constructs mapped to chunk types:
/// - `function_declaration` → [`ChunkType::Function`] or [`ChunkType::Method`]
/// - `test_declaration`     → [`ChunkType::Test`]
/// - `variable_declaration` with struct/enum/union/error initialiser
///   → [`ChunkType::Struct`] / [`ChunkType::Enum`] / [`ChunkType::ErrorSet`]
///
/// Note: in tree-sitter-zig the grammar node names may vary by version.
/// The classifier tries multiple candidate names for robustness.
fn classify_zig(
    kind: &str,
    node: &Node<'_>,
    source: &[u8],
    parent_kind: Option<&str>,
) -> Option<(ChunkType, Option<String>)> {
    match kind {
        // ── Functions ──────────────────────────────────────────────────────
        "function_declaration" => {
            // Name is the first `identifier` child (field_text works since the
            // grammar exposes it as the `name` field in tree-sitter-zig 1.x).
            let name =
                field_text(node, "name", source).or_else(|| zig_first_identifier(node, source));
            // When a function_declaration is nested inside a variable_declaration
            // (i.e. inside a struct/enum/union body), classify it as a method.
            let chunk_type = if parent_kind
                .is_some_and(|k| k == "variable_declaration" || k.contains("struct"))
            {
                ChunkType::Method
            } else {
                ChunkType::Function
            };
            Some((chunk_type, name))
        }
        // ── Tests ──────────────────────────────────────────────────────────
        "test_declaration" => {
            // Test name is inside a `string` → `string_content` child.
            let name = zig_test_name(node, source);
            Some((ChunkType::Test, name))
        }
        // ── Named type declarations via `const Name = <type> { … }` ───────
        "variable_declaration" => {
            // Only chunk const declarations that initialise a container type.
            // The declared name is in the first `identifier` child.
            let name = zig_first_identifier(node, source);
            let chunk_type = zig_container_kind(node)?;
            Some((chunk_type, name))
        }
        _ => None,
    }
}

/// Infer the chunk type from a Zig variable declaration's initialiser value.
///
/// Returns `Some(...)` only when the value is a container literal (struct,
/// enum, union) or an error set.  Returns `None` for plain value assignments.
fn zig_container_kind(decl: &Node<'_>) -> Option<ChunkType> {
    let mut cursor = decl.walk();
    for child in decl.children(&mut cursor) {
        match child.kind() {
            "struct_declaration" => return Some(ChunkType::Struct),
            "enum_declaration" => return Some(ChunkType::Enum),
            "union_declaration" => return Some(ChunkType::Struct), // unions → Struct
            "error_set_declaration" => return Some(ChunkType::ErrorSet),
            _ => {}
        }
    }
    None
}

/// Extract the test name from a Zig `test "…" { }` declaration.
///
/// In tree-sitter-zig 1.x the string node structure is:
/// `test_declaration → string → string_content`.
fn zig_test_name(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            // Look for the string_content grandchild.
            let mut inner = child.walk();
            for gc in child.children(&mut inner) {
                if gc.kind() == "string_content" {
                    if let Ok(text) = gc.utf8_text(source) {
                        let s = text.trim().to_string();
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
            // Fallback: use the whole string node text stripped of quotes.
            if let Ok(text) = child.utf8_text(source) {
                let s = text.trim().trim_matches('"').to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Returns `true` when a Zig `const x = @import("…")` declaration is an import.
fn zig_is_import_decl(node: &Node<'_>, source: &[u8]) -> bool {
    let text = node.utf8_text(source).unwrap_or("");
    text.contains("@import(")
}

/// Return the text of the first `IDENTIFIER` / `identifier` direct child.
fn zig_first_identifier(node: &Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "IDENTIFIER" || child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(source) {
                let s = text.trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
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
        Language::Swift => {
            // Swift access modifiers appear as `modifiers` or directly as
            // `visibility_modifier` / `access_level_modifier` children.
            // Access levels: open > public > internal (default) > fileprivate > private
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let child_kind = child.kind();
                // Direct access-level modifier node.
                if child_kind == "visibility_modifier" || child_kind == "access_level_modifier" {
                    return Some(swift_visibility_from_text(child.utf8_text(source).unwrap_or("")));
                }
                // Grouped modifiers container — scan its children.
                if child_kind == "modifiers" {
                    let mut mod_cursor = child.walk();
                    for modifier in child.children(&mut mod_cursor) {
                        let mk = modifier.kind();
                        if mk == "visibility_modifier" || mk == "access_level_modifier" {
                            return Some(swift_visibility_from_text(
                                modifier.utf8_text(source).unwrap_or(""),
                            ));
                        }
                        // Some grammars embed the keyword directly.
                        let text = modifier.utf8_text(source).unwrap_or("").trim();
                        if matches!(
                            text,
                            "open" | "public" | "internal" | "fileprivate" | "private"
                        ) {
                            return Some(swift_visibility_from_text(text));
                        }
                    }
                }
            }
            // Swift default visibility is `internal` — closest mapping is Module.
            Some(Visibility::Module)
        }
        Language::Zig => {
            // Zig uses `pub` as the sole visibility keyword.
            // Scan children for a `pub` keyword node.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if !child.is_named() && child.utf8_text(source).ok() == Some("pub") {
                    return Some(Visibility::Public);
                }
            }
            Some(Visibility::Private)
        }
        Language::Go => {
            // Go uses capitalisation for export visibility.
            // An identifier starting with an uppercase letter is exported (public).
            let name = symbol_name.as_deref().unwrap_or("");
            if name.starts_with(|c: char| c.is_uppercase()) {
                Some(Visibility::Public)
            } else {
                Some(Visibility::Private)
            }
        }
    }
}

/// Map a Swift access-level keyword to a [`Visibility`] variant.
fn swift_visibility_from_text(text: &str) -> Visibility {
    // Strip any parenthesised setter annotation, e.g. `private(set)`.
    let keyword = text.split('(').next().unwrap_or("").trim();
    match keyword {
        "open" | "public" => Visibility::Public,
        "fileprivate" => Visibility::Protected,
        "private" => Visibility::Private,
        // `internal` and anything unrecognised → module-scoped.
        _ => Visibility::Module,
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
            Language::Swift => {
                // Each `parameter` child has an optional external label and a
                // mandatory internal name.  We prefer the internal name.
                if child.kind() == "parameter" {
                    // Try named fields in order of preference.
                    let name = field_text(&child, "internal_name", source)
                        .or_else(|| field_text(&child, "name", source))
                        .or_else(|| {
                            // Fallback: pick the last simple_identifier before
                            // the colon (the internal name in `label name: Type`).
                            let mut last = None;
                            let mut cursor2 = child.walk();
                            for gc in child.children(&mut cursor2) {
                                if gc.kind() == "simple_identifier" {
                                    last = gc.utf8_text(source).ok().map(|s| s.to_string());
                                } else if gc.utf8_text(source).ok() == Some(":") {
                                    break;
                                }
                            }
                            last
                        })
                        .unwrap_or_default();

                    if name.is_empty() || name == "_" {
                        continue;
                    }

                    let mut cursor2 = child.walk();
                    let type_annotation = field_text(&child, "type", source).or_else(|| {
                        // Some grammar versions put the type in a
                        // `type_annotation` child node.
                        let ta_text = child
                            .children(&mut cursor2)
                            .find(|gc| gc.kind() == "type_annotation")
                            .and_then(|ta| ta.utf8_text(source).ok())
                            .map(|s| s.trim_start_matches(':').trim().to_string());
                        ta_text
                    });

                    params.push(Parameter { name, type_annotation });
                }
            }
            Language::Zig => {
                // Zig parameters are `param_decl` nodes.
                // Fields: `name` (IDENTIFIER) and `type` (the type expression).
                if child.kind() == "param_decl" || child.kind() == "ParamDecl" {
                    let name = field_text(&child, "name", source)
                        .or_else(|| zig_first_identifier(&child, source))
                        .unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let type_annotation = field_text(&child, "type", source);
                    params.push(Parameter { name, type_annotation });
                }
            }
            Language::Go => {
                // `parameter_declaration` holds one or more names plus a type.
                if child.kind() == "parameter_declaration" {
                    let type_annotation = child
                        .child_by_field_name("type")
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(|s| s.to_string());
                    let mut name_cursor = child.walk();
                    for gc in child.children(&mut name_cursor) {
                        if gc.kind() == "identifier" {
                            let name = gc.utf8_text(source).unwrap_or("").to_string();
                            if !name.is_empty() {
                                params.push(Parameter {
                                    name,
                                    type_annotation: type_annotation.clone(),
                                });
                            }
                        }
                    }
                }
            }
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
        Language::Swift => {
            // Swift return type follows `->`.  The grammar may expose it as
            // a `return_type` field or as a child `throws_modifier`/`type` node.
            // Try the named field first.
            if let Some(rt) = field_text(node, "return_type", source) {
                return Some(rt);
            }
            // Fallback: find the node that follows `->` among direct children.
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            let mut found_arrow = false;
            for child in &children {
                if found_arrow {
                    // Skip `throws`/`async` keywords that appear between -> and type.
                    let k = child.kind();
                    if k == "throws_modifier" || k == "async" || k == "rethrows" {
                        continue;
                    }
                    if let Ok(text) = child.utf8_text(source) {
                        let s = text.trim().to_string();
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
                if child.utf8_text(source).ok() == Some("->") {
                    found_arrow = true;
                }
            }
            None
        }
        Language::Zig => {
            // Zig return type follows `fn name(…) ReturnType { … }`.
            // Try the `return_type` named field first.
            if let Some(rt) = field_text(node, "return_type", source) {
                return Some(rt);
            }
            // Fallback: the return type is the token between `)` and `{`.
            // We find the closing `)` of the parameter list, then take the
            // next named sibling as the return type.
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            let mut after_params = false;
            for child in &children {
                let k = child.kind();
                if after_params {
                    // Skip `callconv` and `align` modifiers.
                    if k == "CallConv" || k == "AlignExpr" || k == "block" || k == "Block" {
                        break;
                    }
                    if child.is_named() {
                        if let Ok(text) = child.utf8_text(source) {
                            let s = text.trim().to_string();
                            if !s.is_empty() {
                                return Some(s);
                            }
                        }
                    }
                }
                if k == "param_list" || k == "FnProtoParamList" {
                    after_params = true;
                }
            }
            None
        }
        Language::Go => {
            // `result` field: either a single type or a `parameter_list` of named returns.
            node.child_by_field_name("result")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.trim().to_string())
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
            Language::Swift => child.kind() == "import_declaration",
            // In Zig, `const x = @import("…")` at the top level acts as an import.
            // We detect these as variable_declaration / VarDecl nodes whose init
            // contains a `@import` builtin call.
            Language::Zig => {
                (child.kind() == "variable_declaration" || child.kind() == "VarDecl")
                    && zig_is_import_decl(&child, source)
            }
            Language::Go => child.kind() == "import_declaration",
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

    // ── Swift ─────────────────────────────────────────────────────────────

    /// Representative Swift source covering the major symbol kinds.
    const SWIFT_SOURCE: &str = r#"
import Foundation
import UIKit

protocol Greetable {
    var name: String { get }
    func greet() -> String
}

class Animal: Greetable {
    var name: String

    init(name: String) {
        self.name = name
    }

    func greet() -> String {
        return "Hello, I'm \(name)"
    }

    func speak() -> String {
        return "..."
    }
}

struct Point {
    var x: Double
    var y: Double

    func distance(to other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}

enum Direction {
    case north
    case south
    case east
    case west

    func opposite() -> Direction {
        switch self {
        case .north: return .south
        case .south: return .north
        case .east:  return .west
        case .west:  return .east
        }
    }
}

extension Animal {
    func description() -> String {
        return "Animal: \(name)"
    }
}

public func makeAnimal(name: String) -> Animal {
    return Animal(name: name)
}

func fetchData(from url: String) async throws -> Data {
    fatalError("not implemented")
}
"#;

    #[test]
    fn swift_extracts_protocol() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let protocols: Vec<_> =
            chunks.iter().filter(|c| c.chunk_type == ChunkType::Protocol).collect();
        assert_eq!(protocols.len(), 1, "expected one protocol");
        assert_eq!(protocols[0].symbol_name.as_deref(), Some("Greetable"));
    }

    #[test]
    fn swift_extracts_class() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
        assert_eq!(classes.len(), 1, "expected one class");
        assert_eq!(classes[0].symbol_name.as_deref(), Some("Animal"));
    }

    #[test]
    fn swift_extracts_struct() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
        assert_eq!(structs.len(), 1, "expected one struct");
        assert_eq!(structs[0].symbol_name.as_deref(), Some("Point"));
    }

    #[test]
    fn swift_extracts_enum() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
        assert_eq!(enums.len(), 1, "expected one enum");
        assert_eq!(enums[0].symbol_name.as_deref(), Some("Direction"));
    }

    #[test]
    fn swift_extracts_extension() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let exts: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Extension).collect();
        assert_eq!(exts.len(), 1, "expected one extension");
        assert_eq!(exts[0].symbol_name.as_deref(), Some("Animal"));
    }

    #[test]
    fn swift_extracts_standalone_functions() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        let names: Vec<_> = fns.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(
            names.iter().any(|n| n.as_str() == "makeAnimal"),
            "expected makeAnimal in functions"
        );
        assert!(names.iter().any(|n| n.as_str() == "fetchData"), "expected fetchData in functions");
    }

    #[test]
    fn swift_extracts_methods() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert!(!methods.is_empty(), "expected at least one method");
        let names: Vec<_> = methods.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "greet"), "expected greet method");
        assert!(names.iter().any(|n| n.as_str() == "speak"), "expected speak method");
    }

    #[test]
    fn swift_init_is_method() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        let init_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| {
                c.chunk_type == ChunkType::Method && c.symbol_name.as_deref() == Some("init")
            })
            .collect();
        assert!(!init_chunks.is_empty(), "expected init to be classified as a method");
    }

    #[test]
    fn swift_method_has_parent_symbol() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        // `speak` is only defined on `Animal`, not in the protocol, so unambiguous.
        let speak = chunks
            .iter()
            .find(|c| {
                c.chunk_type == ChunkType::Method && c.symbol_name.as_deref() == Some("speak")
            })
            .expect("speak method should exist");
        assert_eq!(speak.parent_symbol.as_deref(), Some("Animal"));
    }

    #[test]
    fn swift_chunk_path_detects_language() {
        let path = Path::new("App.swift");
        let chunks = chunker().chunk_path(path, "func hello() -> String { \"hello\" }\n").unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].language, Language::Swift);
    }

    #[test]
    fn swift_chunk_line_numbers_are_nonzero() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        for chunk in &chunks {
            assert!(chunk.start_line >= 1, "start_line should be 1-based");
            assert!(chunk.end_line >= chunk.start_line);
        }
    }

    #[test]
    fn swift_imports_attached_to_chunks() {
        let chunks = chunker().chunk("App.swift", SWIFT_SOURCE, Language::Swift).unwrap();
        // Every chunk should have the file-level imports attached.
        for chunk in &chunks {
            assert!(
                !chunk.metadata.imports.is_empty(),
                "chunk '{}' should have imports attached",
                chunk.symbol_name.as_deref().unwrap_or("<anon>")
            );
        }
    }

    // ── Zig ───────────────────────────────────────────────────────────────

    /// Representative Zig source covering the major symbol kinds.
    const ZIG_SOURCE: &str = r#"
const std = @import("std");
const mem = @import("std").mem;

/// Adds two integers.
pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

pub fn main() void {
    std.debug.print("Hello\n", .{});
}

/// A simple struct with a method.
pub const MyStruct = struct {
    value: i32,

    pub fn init(v: i32) MyStruct {
        return MyStruct{ .value = v };
    }

    pub fn getValue(self: MyStruct) i32 {
        return self.value;
    }
};

/// Colour enum.
pub const Color = enum {
    red,
    green,
    blue,
};

/// Tagged union.
pub const MyUnion = union(enum) {
    int: i32,
    float: f64,
};

/// Error set.
pub const MyError = error {
    OutOfMemory,
    InvalidInput,
};

test "basic addition" {
    const result = add(1, 2);
    try std.testing.expectEqual(result, 3);
}

test "struct init" {
    const s = MyStruct.init(42);
    try std.testing.expectEqual(s.getValue(), 42);
}
"#;

    #[test]
    fn zig_extracts_functions() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        let names: Vec<_> = fns.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "add"), "expected add function");
        assert!(names.iter().any(|n| n.as_str() == "main"), "expected main function");
    }

    #[test]
    fn zig_extracts_struct() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
        assert!(!structs.is_empty(), "expected at least one struct");
        let names: Vec<_> = structs.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "MyStruct"), "expected MyStruct");
    }

    #[test]
    fn zig_extracts_union_as_struct() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
        let names: Vec<_> = structs.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "MyUnion"), "expected MyUnion as struct");
    }

    #[test]
    fn zig_extracts_enum() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
        assert_eq!(enums.len(), 1, "expected one enum");
        assert_eq!(enums[0].symbol_name.as_deref(), Some("Color"));
    }

    #[test]
    fn zig_extracts_error_set() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let errors: Vec<_> =
            chunks.iter().filter(|c| c.chunk_type == ChunkType::ErrorSet).collect();
        assert_eq!(errors.len(), 1, "expected one error set");
        assert_eq!(errors[0].symbol_name.as_deref(), Some("MyError"));
    }

    #[test]
    fn zig_extracts_tests() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let tests: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Test).collect();
        assert_eq!(tests.len(), 2, "expected two test declarations");
        let names: Vec<_> = tests.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(
            names.iter().any(|n| n.as_str() == "basic addition"),
            "expected 'basic addition' test"
        );
    }

    #[test]
    fn zig_extracts_methods() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert!(!methods.is_empty(), "expected methods inside struct");
        let names: Vec<_> = methods.iter().flat_map(|c| &c.symbol_name).collect();
        assert!(names.iter().any(|n| n.as_str() == "init"), "expected init method");
        assert!(names.iter().any(|n| n.as_str() == "getValue"), "expected getValue method");
    }

    #[test]
    fn zig_method_has_parent_symbol() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let init =
            chunks.iter().find(|c| c.symbol_name.as_deref() == Some("init")).expect("init chunk");
        assert_eq!(init.parent_symbol.as_deref(), Some("MyStruct"));
    }

    #[test]
    fn zig_chunk_path_detects_language() {
        let path = Path::new("main.zig");
        let chunks = chunker().chunk_path(path, "pub fn hello() void {}\n").unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].language, Language::Zig);
    }

    #[test]
    fn zig_chunk_line_numbers_are_nonzero() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        for chunk in &chunks {
            assert!(chunk.start_line >= 1, "start_line should be 1-based");
            assert!(chunk.end_line >= chunk.start_line);
        }
    }

    #[test]
    fn zig_imports_attached_to_chunks() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        // Every chunk should have the file-level @import declarations attached.
        for chunk in &chunks {
            assert!(
                !chunk.metadata.imports.is_empty(),
                "chunk '{}' should have imports attached",
                chunk.symbol_name.as_deref().unwrap_or("<anon>")
            );
        }
    }

    #[test]
    fn zig_public_function_is_public() {
        let chunks = chunker().chunk("main.zig", ZIG_SOURCE, Language::Zig).unwrap();
        let add =
            chunks.iter().find(|c| c.symbol_name.as_deref() == Some("add")).expect("add chunk");
        assert_eq!(add.metadata.visibility, Some(crate::metadata::Visibility::Public));
    }

    // ── Go ────────────────────────────────────────────────────────────────

    const GO_SOURCE: &str = r#"package main

import (
	"fmt"
	"strings"
)

// Add adds two integers and returns the result.
func Add(a, b int) int {
	return a + b
}

// Format formats a string.
func Format(s string) string {
	return strings.TrimSpace(s)
}

func (r *Receiver) Method() string {
	return fmt.Sprintf("receiver")
}

type Point struct {
	X, Y float64
}

type Config struct {
	Name    string
	Enabled bool
}

type Stringer interface {
	String() string
}

type ReadWriter interface {
	Read() string
	Write(s string)
}
"#;

    #[test]
    fn go_extracts_functions() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
        assert!(
            fns.iter().any(|c| c.symbol_name.as_deref() == Some("Add")),
            "should extract Add function"
        );
        assert!(
            fns.iter().any(|c| c.symbol_name.as_deref() == Some("Format")),
            "should extract Format function"
        );
    }

    #[test]
    fn go_extracts_method() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
        assert!(!methods.is_empty(), "should extract at least one method");
        assert!(
            methods.iter().any(|c| c.symbol_name.as_deref() == Some("Method")),
            "should extract Method"
        );
    }

    #[test]
    fn go_extracts_structs() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
        assert!(
            structs.iter().any(|c| c.symbol_name.as_deref() == Some("Point")),
            "should extract Point struct"
        );
        assert!(
            structs.iter().any(|c| c.symbol_name.as_deref() == Some("Config")),
            "should extract Config struct"
        );
    }

    #[test]
    fn go_extracts_interfaces() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let interfaces: Vec<_> =
            chunks.iter().filter(|c| c.chunk_type == ChunkType::Trait).collect();
        assert!(
            interfaces.iter().any(|c| c.symbol_name.as_deref() == Some("Stringer")),
            "should extract Stringer interface"
        );
        assert!(
            interfaces.iter().any(|c| c.symbol_name.as_deref() == Some("ReadWriter")),
            "should extract ReadWriter interface"
        );
    }

    #[test]
    fn go_exported_symbol_is_public() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let add = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("Add")).unwrap();
        assert_eq!(add.metadata.visibility, Some(crate::metadata::Visibility::Public));
    }

    #[test]
    fn go_unexported_symbol_is_private() {
        let source = "package main\nfunc privateHelper() {}\n";
        let chunks = chunker().chunk("main.go", source, Language::Go).unwrap();
        let f = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("privateHelper")).unwrap();
        assert_eq!(f.metadata.visibility, Some(crate::metadata::Visibility::Private));
    }

    #[test]
    fn go_function_extracts_parameters() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let add = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("Add")).unwrap();
        let params = &add.metadata.parameters;
        assert_eq!(params.len(), 2, "Add should have 2 parameters");
        let names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"a"), "should include param 'a'");
        assert!(names.contains(&"b"), "should include param 'b'");
    }

    #[test]
    fn go_function_extracts_return_type() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let add = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("Add")).unwrap();
        assert_eq!(add.metadata.return_type.as_deref(), Some("int"));
    }

    #[test]
    fn go_chunk_line_numbers_are_nonzero() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        for chunk in &chunks {
            assert!(chunk.start_line >= 1, "start_line should be 1-based");
            assert!(chunk.end_line >= chunk.start_line);
        }
    }

    #[test]
    fn go_chunk_path_via_extension() {
        let path = Path::new("pkg/util.go");
        let source = "package util\nfunc Helper() {}\n";
        let chunks = chunker().chunk_path(path, source).unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].language, Language::Go);
    }

    #[test]
    fn go_import_declaration_collected() {
        let chunks = chunker().chunk("main.go", GO_SOURCE, Language::Go).unwrap();
        let fn_chunk = chunks.iter().find(|c| c.symbol_name.as_deref() == Some("Add")).unwrap();
        assert!(!fn_chunk.metadata.imports.is_empty(), "imports should be attached to chunks");
        let imports_text = fn_chunk.metadata.imports.join("\n");
        assert!(imports_text.contains("fmt"), "should include fmt import");
    }
}
