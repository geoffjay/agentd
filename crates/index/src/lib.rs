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

pub mod api;
pub mod chunking;
pub mod config;
pub mod error;
