//! agentd-index — code index service library.
//!
//! Provides code indexing and semantic search over repositories.
//! This crate contains the library components; the binary entry point
//! is `src/main.rs`.
//!
//! # Modules
//!
//! - [`config`] — service configuration loaded from environment variables
//! - [`error`] — error types using `thiserror`
//! - [`api`] — Axum HTTP route handlers
//! - [`chunking`] — tree-sitter / semantic / hierarchical chunking pipeline
//! - [`search`] — pluggable search strategies (vector, hybrid, rerank)
//! - [`store`] — LanceDB vector store and embedding service

pub mod api;
pub mod chunking;
pub mod config;
pub mod dependencies;
pub mod enrichment;
pub mod error;
pub mod indexer;
pub mod metadata;
pub mod search;
pub mod store;
