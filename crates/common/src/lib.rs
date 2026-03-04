//! Shared types and utilities for agentd services.
//!
//! This crate provides common functionality used across multiple agentd service
//! crates, reducing duplication and ensuring consistency.
//!
//! # Modules
//!
//! - **types** — Shared API types: `PaginatedResponse<T>`, `HealthResponse`,
//!   pagination helpers (`clamp_limit`, `DEFAULT_PAGE_LIMIT`)
//!
//! - **error** — Shared API error types with `IntoResponse` implementations
//!   for consistent HTTP error responses across all services
//!
//! - **client** — Generic `ServiceClient` base providing typed HTTP methods
//!   (`get`, `post`, `put`, `delete`) with consistent error handling
//!
//! - **storage** — SQLite utilities: database path resolution, connection pool
//!   creation, and test helpers
//!
//! - **server** — Server initialization helpers: `init_tracing()` for
//!   structured logging setup, and common middleware configuration
//!
//! # Usage
//!
//! Add to your crate's `Cargo.toml`:
//!
//! ```toml
//! agentd-common = { path = "../common" }
//! ```
//!
//! Then import the modules you need:
//!
//! ```rust,ignore
//! use agentd_common::types::PaginatedResponse;
//! use agentd_common::error::ApiError;
//! ```
//!
//! # Migration Plan
//!
//! Each module is populated incrementally as individual crates migrate
//! their duplicated code here. See the following issues for details:
//!
//! - #45 — Extract `PaginatedResponse` and pagination helpers → `types`
//! - #48 — Deduplicate `ApiError` → `error`
//! - #49 — Extract HTTP client base → `client`
//! - #46 — Extract SQLite utilities → `storage`
//! - #47 — Extract tracing/server init → `server`

pub mod client;
pub mod error;
pub mod server;
pub mod storage;
pub mod types;
