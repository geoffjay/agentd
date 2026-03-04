//! Shared API error types with axum `IntoResponse` implementations.
//!
//! This module will contain:
//! - `ApiError` — base error enum (NotFound, InvalidInput, Internal, etc.)
//! - `IntoResponse` impl for consistent HTTP status code mapping
//! - Error conversion traits (`From<anyhow::Error>`, etc.)
//!
//! See #48 for migration details.
