//! Shared API types used across agentd services.
//!
//! This module will contain:
//! - `PaginatedResponse<T>` — generic paginated list response envelope
//! - `HealthResponse` — standardized health check response
//! - `clamp_limit()` — pagination limit clamping utility
//! - `DEFAULT_PAGE_LIMIT`, `MAX_PAGE_LIMIT` — pagination constants
//!
//! See #45 and #80 for migration details.
