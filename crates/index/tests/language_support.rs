//! Integration tests for the language support test harness (issue #974).
//!
//! These tests validate that the [`SyntacticChunker`] correctly parses each
//! supported language fixture file, extracts the expected symbols, handles edge
//! cases gracefully, and produces chunk data that is schema-aligned for storage
//! in LanceDB.
//!
//! Fixture files live in `tests/fixtures/` and are embedded at compile time via
//! [`include_str!`].  Each fixture contains representative source code that
//! exercises the major symbol kinds for its language.

use index::chunking::types::{ChunkType, HierarchyLevel, Language};
use index::chunking::{Chunker, SyntacticChunker};
use std::path::Path;

// ── Fixture source ────────────────────────────────────────────────────────────

const RUST_FIXTURE: &str = include_str!("fixtures/sample.rs");
const PYTHON_FIXTURE: &str = include_str!("fixtures/sample.py");
const JS_FIXTURE: &str = include_str!("fixtures/sample.js");
const TS_FIXTURE: &str = include_str!("fixtures/sample.ts");
const SWIFT_FIXTURE: &str = include_str!("fixtures/sample.swift");
const ZIG_FIXTURE: &str = include_str!("fixtures/sample.zig");
const GO_FIXTURE: &str = include_str!("fixtures/sample.go");

fn chunker() -> SyntacticChunker {
    SyntacticChunker::new()
}

// ── Parser stability — no panics on fixture input ─────────────────────────────

#[test]
fn rust_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust);
    assert!(result.is_ok(), "Rust fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn python_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.py", PYTHON_FIXTURE, Language::Python);
    assert!(result.is_ok(), "Python fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn js_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.js", JS_FIXTURE, Language::JavaScript);
    assert!(result.is_ok(), "JavaScript fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn ts_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.ts", TS_FIXTURE, Language::TypeScript);
    assert!(result.is_ok(), "TypeScript fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn swift_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift);
    assert!(result.is_ok(), "Swift fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn zig_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig);
    assert!(result.is_ok(), "Zig fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

#[test]
fn go_fixture_parses_without_panic() {
    let result = chunker().chunk("sample.go", GO_FIXTURE, Language::Go);
    assert!(result.is_ok(), "Go fixture parse failed: {:?}", result.err());
    assert!(!result.unwrap().is_empty(), "expected at least one chunk");
}

// ── chunk_path — file-extension language detection ────────────────────────────

#[test]
fn rust_detected_by_rs_extension() {
    let path = Path::new("tests/fixtures/sample.rs");
    let chunks = chunker().chunk_path(path, RUST_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::Rust));
}

#[test]
fn python_detected_by_py_extension() {
    let path = Path::new("tests/fixtures/sample.py");
    let chunks = chunker().chunk_path(path, PYTHON_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::Python));
}

#[test]
fn js_detected_by_js_extension() {
    let path = Path::new("tests/fixtures/sample.js");
    let chunks = chunker().chunk_path(path, JS_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::JavaScript));
}

#[test]
fn ts_detected_by_ts_extension() {
    let path = Path::new("tests/fixtures/sample.ts");
    let chunks = chunker().chunk_path(path, TS_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::TypeScript));
}

#[test]
fn swift_detected_by_swift_extension() {
    let path = Path::new("tests/fixtures/sample.swift");
    let chunks = chunker().chunk_path(path, SWIFT_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::Swift));
}

#[test]
fn zig_detected_by_zig_extension() {
    let path = Path::new("tests/fixtures/sample.zig");
    let chunks = chunker().chunk_path(path, ZIG_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::Zig));
}

#[test]
fn go_detected_by_go_extension() {
    let path = Path::new("tests/fixtures/sample.go");
    let chunks = chunker().chunk_path(path, GO_FIXTURE).unwrap();
    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|c| c.language == Language::Go));
}

#[test]
fn js_detected_by_mjs_extension() {
    let path = Path::new("module.mjs");
    let chunks = chunker().chunk_path(path, "export function hello() { return 1; }\n").unwrap();
    assert!(chunks.iter().all(|c| c.language == Language::JavaScript));
}

#[test]
fn ts_detected_by_mts_extension() {
    let path = Path::new("module.mts");
    let chunks =
        chunker().chunk_path(path, "export function hello(): number { return 1; }\n").unwrap();
    assert!(chunks.iter().all(|c| c.language == Language::TypeScript));
}

#[test]
fn unknown_extension_returns_error() {
    let path = Path::new("data.csv");
    assert!(
        chunker().chunk_path(path, "a,b,c\n1,2,3\n").is_err(),
        "unrecognized extension should produce an error"
    );
}

// ── Symbol extraction from fixture files ──────────────────────────────────────

#[test]
fn rust_fixture_extracts_struct() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
    assert!(!structs.is_empty(), "expected at least one struct");
    assert!(
        structs.iter().any(|c| c.symbol_name.as_deref() == Some("Point")),
        "expected Point struct"
    );
}

#[test]
fn rust_fixture_extracts_impl() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let impls: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Impl).collect();
    assert!(!impls.is_empty(), "expected at least one impl block");
}

#[test]
fn rust_fixture_extracts_trait() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let traits: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Trait).collect();
    assert!(!traits.is_empty(), "expected at least one trait");
    assert!(
        traits.iter().any(|c| c.symbol_name.as_deref() == Some("Shape")),
        "expected Shape trait"
    );
}

#[test]
fn rust_fixture_extracts_enum() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
    assert!(!enums.is_empty(), "expected at least one enum");
    assert!(enums.iter().any(|c| c.symbol_name.as_deref() == Some("Color")), "expected Color enum");
}

#[test]
fn rust_fixture_extracts_module() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let mods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Module).collect();
    assert!(!mods.is_empty(), "expected at least one module");
    assert!(
        mods.iter().any(|c| c.symbol_name.as_deref() == Some("geometry")),
        "expected geometry module"
    );
}

#[test]
fn rust_fixture_extracts_function() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("word_count")),
        "expected word_count function"
    );
}

#[test]
fn python_fixture_extracts_class() {
    let chunks = chunker().chunk("sample.py", PYTHON_FIXTURE, Language::Python).unwrap();
    let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
    assert!(!classes.is_empty(), "expected at least one class");
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("Animal")),
        "expected Animal class"
    );
    assert!(classes.iter().any(|c| c.symbol_name.as_deref() == Some("Dog")), "expected Dog class");
}

#[test]
fn python_fixture_extracts_methods() {
    let chunks = chunker().chunk("sample.py", PYTHON_FIXTURE, Language::Python).unwrap();
    let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
    assert!(!methods.is_empty(), "expected at least one method");
    assert!(
        methods.iter().any(|c| c.symbol_name.as_deref() == Some("speak")),
        "expected speak method"
    );
    assert!(
        methods.iter().any(|c| c.symbol_name.as_deref() == Some("fetch")),
        "expected fetch method"
    );
}

#[test]
fn python_fixture_extracts_functions() {
    let chunks = chunker().chunk("sample.py", PYTHON_FIXTURE, Language::Python).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("greet")),
        "expected greet function"
    );
}

#[test]
fn js_fixture_extracts_classes() {
    let chunks = chunker().chunk("sample.js", JS_FIXTURE, Language::JavaScript).unwrap();
    let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
    assert!(!classes.is_empty(), "expected at least one class");
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("Calculator")),
        "expected Calculator class"
    );
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("EventBus")),
        "expected EventBus class"
    );
}

#[test]
fn js_fixture_extracts_function() {
    let chunks = chunker().chunk("sample.js", JS_FIXTURE, Language::JavaScript).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("greet")),
        "expected greet function"
    );
}

#[test]
fn ts_fixture_extracts_classes() {
    let chunks = chunker().chunk("sample.ts", TS_FIXTURE, Language::TypeScript).unwrap();
    let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
    assert!(!classes.is_empty(), "expected at least one class");
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("UserService")),
        "expected UserService class"
    );
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("Logger")),
        "expected Logger class"
    );
}

#[test]
fn ts_fixture_extracts_function() {
    let chunks = chunker().chunk("sample.ts", TS_FIXTURE, Language::TypeScript).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("identity")),
        "expected identity function"
    );
}

#[test]
fn swift_fixture_extracts_protocols() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    let protocols: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Protocol).collect();
    assert!(!protocols.is_empty(), "expected at least one protocol");
    assert!(
        protocols.iter().any(|c| c.symbol_name.as_deref() == Some("Drawable")),
        "expected Drawable protocol"
    );
}

#[test]
fn swift_fixture_extracts_class() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    let classes: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Class).collect();
    assert!(
        classes.iter().any(|c| c.symbol_name.as_deref() == Some("Shape")),
        "expected Shape class"
    );
}

#[test]
fn swift_fixture_extracts_structs() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
    assert!(!structs.is_empty(), "expected at least one struct");
    assert!(
        structs.iter().any(|c| c.symbol_name.as_deref() == Some("Circle")),
        "expected Circle struct"
    );
}

#[test]
fn swift_fixture_extracts_extension() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    let exts: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Extension).collect();
    assert!(!exts.is_empty(), "expected at least one extension");
    assert!(
        exts.iter().any(|c| c.symbol_name.as_deref() == Some("Circle")),
        "expected Circle extension"
    );
}

#[test]
fn swift_fixture_extracts_enum() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
    assert!(
        enums.iter().any(|c| c.symbol_name.as_deref() == Some("Direction")),
        "expected Direction enum"
    );
}

#[test]
fn zig_fixture_extracts_struct() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
    assert!(
        structs.iter().any(|c| c.symbol_name.as_deref() == Some("Stack")),
        "expected Stack struct"
    );
}

#[test]
fn zig_fixture_extracts_enum() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let enums: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Enum).collect();
    assert!(
        enums.iter().any(|c| c.symbol_name.as_deref() == Some("Direction")),
        "expected Direction enum"
    );
}

#[test]
fn zig_fixture_extracts_error_set() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let errors: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::ErrorSet).collect();
    assert!(
        errors.iter().any(|c| c.symbol_name.as_deref() == Some("ParseError")),
        "expected ParseError error set"
    );
}

#[test]
fn zig_fixture_extracts_tests() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let tests: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Test).collect();
    assert!(tests.len() >= 3, "expected at least 3 test declarations; got {}", tests.len());
    assert!(
        tests.iter().any(|c| c.symbol_name.as_deref() == Some("stack push and pop")),
        "expected 'stack push and pop' test"
    );
}

#[test]
fn zig_fixture_extracts_functions() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(fns.iter().any(|c| c.symbol_name.as_deref() == Some("add")), "expected add function");
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("clamp")),
        "expected clamp function"
    );
}

#[test]
fn go_fixture_extracts_structs() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    let structs: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Struct).collect();
    assert!(
        structs.iter().any(|c| c.symbol_name.as_deref() == Some("Circle")),
        "expected Circle struct"
    );
    assert!(
        structs.iter().any(|c| c.symbol_name.as_deref() == Some("Rectangle")),
        "expected Rectangle struct"
    );
}

#[test]
fn go_fixture_extracts_interface() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    let traits: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Trait).collect();
    assert!(
        traits.iter().any(|c| c.symbol_name.as_deref() == Some("Shape")),
        "expected Shape interface"
    );
}

#[test]
fn go_fixture_extracts_methods() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    let methods: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Method).collect();
    assert!(!methods.is_empty(), "expected at least one method");
}

#[test]
fn go_fixture_extracts_functions() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert!(
        fns.iter().any(|c| c.symbol_name.as_deref() == Some("NewCircle")),
        "expected NewCircle function"
    );
}

// ── Hierarchy level — all syntactic chunks must be Symbol ─────────────────────

#[test]
fn rust_chunks_are_symbol_level() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    for chunk in &chunks {
        assert_eq!(
            chunk.hierarchy_level,
            HierarchyLevel::Symbol,
            "chunk '{}' should be Symbol level",
            chunk.symbol_name.as_deref().unwrap_or("<anon>")
        );
    }
}

#[test]
fn python_chunks_are_symbol_level() {
    let chunks = chunker().chunk("sample.py", PYTHON_FIXTURE, Language::Python).unwrap();
    for chunk in &chunks {
        assert_eq!(chunk.hierarchy_level, HierarchyLevel::Symbol);
    }
}

#[test]
fn swift_chunks_are_symbol_level() {
    let chunks = chunker().chunk("sample.swift", SWIFT_FIXTURE, Language::Swift).unwrap();
    for chunk in &chunks {
        assert_eq!(chunk.hierarchy_level, HierarchyLevel::Symbol);
    }
}

#[test]
fn zig_chunks_are_symbol_level() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    for chunk in &chunks {
        assert_eq!(chunk.hierarchy_level, HierarchyLevel::Symbol);
    }
}

#[test]
fn go_chunks_are_symbol_level() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    for chunk in &chunks {
        assert_eq!(chunk.hierarchy_level, HierarchyLevel::Symbol);
    }
}

// ── Line number validity ──────────────────────────────────────────────────────

#[test]
fn rust_line_numbers_are_valid() {
    let chunks = chunker().chunk("sample.rs", RUST_FIXTURE, Language::Rust).unwrap();
    let total_lines = RUST_FIXTURE.lines().count();
    for chunk in &chunks {
        assert!(chunk.start_line >= 1, "start_line must be 1-based");
        assert!(chunk.end_line >= chunk.start_line, "end_line must be >= start_line");
        assert!(
            chunk.end_line <= total_lines,
            "end_line {} exceeds file length {}",
            chunk.end_line,
            total_lines
        );
    }
}

#[test]
fn go_line_numbers_are_valid() {
    let chunks = chunker().chunk("sample.go", GO_FIXTURE, Language::Go).unwrap();
    let total_lines = GO_FIXTURE.lines().count();
    for chunk in &chunks {
        assert!(chunk.start_line >= 1);
        assert!(chunk.end_line >= chunk.start_line);
        assert!(chunk.end_line <= total_lines);
    }
}

#[test]
fn zig_line_numbers_are_valid() {
    let chunks = chunker().chunk("sample.zig", ZIG_FIXTURE, Language::Zig).unwrap();
    let total_lines = ZIG_FIXTURE.lines().count();
    for chunk in &chunks {
        assert!(chunk.start_line >= 1);
        assert!(chunk.end_line >= chunk.start_line);
        assert!(chunk.end_line <= total_lines);
    }
}

// ── Schema alignment — ChunkType serializes to valid snake_case strings ───────

#[test]
fn chunk_types_serialize_to_snake_case() {
    use serde_json::Value;

    // All ChunkType variants and their expected LanceDB schema values.
    let cases: &[(ChunkType, &str)] = &[
        (ChunkType::Function, "\"function\""),
        (ChunkType::Class, "\"class\""),
        (ChunkType::Method, "\"method\""),
        (ChunkType::Struct, "\"struct\""),
        (ChunkType::Enum, "\"enum\""),
        (ChunkType::Trait, "\"trait\""),
        (ChunkType::Impl, "\"impl\""),
        (ChunkType::Module, "\"module\""),
        (ChunkType::Protocol, "\"protocol\""),
        (ChunkType::Extension, "\"extension\""),
        (ChunkType::Test, "\"test\""),
        (ChunkType::ErrorSet, "\"error_set\""),
    ];

    for (chunk_type, expected_json) in cases {
        let serialized = serde_json::to_string(chunk_type).unwrap();
        assert_eq!(
            serialized, *expected_json,
            "ChunkType::{:?} serialized incorrectly",
            chunk_type
        );
        // Round-trip: deserialize back to the same variant.
        let deserialized: ChunkType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, *chunk_type);
    }
}

#[test]
fn language_serializes_to_lowercase() {
    let cases: &[(Language, &str)] = &[
        (Language::Rust, "\"rust\""),
        (Language::Python, "\"python\""),
        (Language::JavaScript, "\"javascript\""),
        (Language::TypeScript, "\"typescript\""),
        (Language::Swift, "\"swift\""),
        (Language::Zig, "\"zig\""),
        (Language::Go, "\"go\""),
    ];
    for (lang, expected_json) in cases {
        let serialized = serde_json::to_string(lang).unwrap();
        assert_eq!(serialized, *expected_json, "Language::{:?} serialized incorrectly", lang);
        let deserialized: Language = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, *lang);
    }
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_rust_file_returns_no_chunks() {
    let chunks = chunker().chunk("empty.rs", "", Language::Rust).unwrap();
    assert!(chunks.is_empty(), "empty file should produce no chunks");
}

#[test]
fn empty_python_file_returns_no_chunks() {
    let chunks = chunker().chunk("empty.py", "", Language::Python).unwrap();
    assert!(chunks.is_empty());
}

#[test]
fn empty_go_file_returns_no_chunks() {
    let chunks = chunker().chunk("empty.go", "", Language::Go).unwrap();
    assert!(chunks.is_empty());
}

#[test]
fn empty_zig_file_returns_no_chunks() {
    let chunks = chunker().chunk("empty.zig", "", Language::Zig).unwrap();
    assert!(chunks.is_empty());
}

#[test]
fn whitespace_only_rust_file_returns_no_chunks() {
    let chunks = chunker().chunk("blank.rs", "   \n\n\t\n", Language::Rust).unwrap();
    assert!(chunks.is_empty(), "whitespace-only file should produce no chunks");
}

#[test]
fn comment_only_rust_file_returns_no_chunks() {
    let source = "// This file has only comments.\n// No symbols here.\n";
    let chunks = chunker().chunk("comments.rs", source, Language::Rust).unwrap();
    assert!(chunks.is_empty(), "comment-only file should produce no chunks");
}

#[test]
fn malformed_rust_does_not_panic() {
    // tree-sitter is error-tolerant and should not panic on broken input.
    let sources =
        ["fn (", "pub struct {", "impl Foo Bar Baz { fn", "<<<MERGE CONFLICT>>>", "✂️ cut here ✂️"];
    for source in sources {
        let result = chunker().chunk("bad.rs", source, Language::Rust);
        // We don't require success — just no panic and no undefined behavior.
        let _ = result;
    }
}

#[test]
fn malformed_python_does_not_panic() {
    let sources = ["def (:", "class :", "import", "def foo(: pass"];
    for source in sources {
        let _ = chunker().chunk("bad.py", source, Language::Python);
    }
}

#[test]
fn malformed_go_does_not_panic() {
    let sources = ["func (", "type {", "package"];
    for source in sources {
        let _ = chunker().chunk("bad.go", source, Language::Go);
    }
}

// ── Large file handling ───────────────────────────────────────────────────────

/// Generates a Rust source file with `n` top-level functions.
fn generate_large_rust_source(n: usize) -> String {
    let mut out = String::new();
    for i in 0..n {
        out.push_str(&format!("pub fn func_{i}(x: i32) -> i32 {{ x + {i} }}\n"));
    }
    out
}

/// Generates a Python source file with `n` top-level functions.
fn generate_large_python_source(n: usize) -> String {
    let mut out = String::new();
    for i in 0..n {
        out.push_str(&format!("def func_{i}(x):\n    return x + {i}\n\n"));
    }
    out
}

#[test]
fn large_rust_file_extracts_all_functions() {
    let n = 200;
    let source = generate_large_rust_source(n);
    let chunks = chunker().chunk("large.rs", &source, Language::Rust).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert_eq!(fns.len(), n, "expected {n} functions; got {}", fns.len());
}

#[test]
fn large_python_file_extracts_all_functions() {
    let n = 200;
    let source = generate_large_python_source(n);
    let chunks = chunker().chunk("large.py", &source, Language::Python).unwrap();
    let fns: Vec<_> = chunks.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    assert_eq!(fns.len(), n, "expected {n} functions; got {}", fns.len());
}

// ── Incremental re-indexing simulation ───────────────────────────────────────

/// Simulates re-chunking a modified file and verifies that the updated chunks
/// reflect the change.  This is the pattern used by the incremental indexer
/// when a file watcher reports a modification.
#[test]
fn rust_rechunking_after_modification_reflects_change() {
    let original = "pub fn add(a: i32, b: i32) -> i32 { a + b }\n";
    let modified = "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\
                    pub fn sub(a: i32, b: i32) -> i32 { a - b }\n";

    let c = chunker();
    let chunks_v1 = c.chunk("math.rs", original, Language::Rust).unwrap();
    let chunks_v2 = c.chunk("math.rs", modified, Language::Rust).unwrap();

    let fns_v1: Vec<_> = chunks_v1.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    let fns_v2: Vec<_> = chunks_v2.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();

    assert_eq!(fns_v1.len(), 1, "original should have one function");
    assert_eq!(fns_v2.len(), 2, "modified version should have two functions");
    assert!(
        fns_v2.iter().any(|c| c.symbol_name.as_deref() == Some("sub")),
        "new function 'sub' should appear after modification"
    );
}

#[test]
fn go_rechunking_after_modification_reflects_change() {
    let original = "package p\nfunc Hello() string { return \"hello\" }\n";
    let modified = "package p\n\
                    func Hello() string { return \"hello\" }\n\
                    func World() string { return \"world\" }\n";

    let c = chunker();
    let chunks_v1 = c.chunk("greet.go", original, Language::Go).unwrap();
    let chunks_v2 = c.chunk("greet.go", modified, Language::Go).unwrap();

    let fns_v1: Vec<_> = chunks_v1.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();
    let fns_v2: Vec<_> = chunks_v2.iter().filter(|c| c.chunk_type == ChunkType::Function).collect();

    assert_eq!(fns_v1.len(), 1);
    assert_eq!(fns_v2.len(), 2);
    assert!(fns_v2.iter().any(|c| c.symbol_name.as_deref() == Some("World")));
}

#[test]
fn python_rechunking_after_modification_reflects_change() {
    let original = "def hello():\n    return 'hello'\n";
    let modified = "def hello():\n    return 'hello'\n\ndef world():\n    return 'world'\n";

    let c = chunker();
    let chunks_v1 = c.chunk("greet.py", original, Language::Python).unwrap();
    let chunks_v2 = c.chunk("greet.py", modified, Language::Python).unwrap();

    assert_eq!(chunks_v1.iter().filter(|c| c.chunk_type == ChunkType::Function).count(), 1);
    assert_eq!(chunks_v2.iter().filter(|c| c.chunk_type == ChunkType::Function).count(), 2);
}
