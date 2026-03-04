//! Server initialization and tracing setup helpers.
//!
//! This module will contain:
//! - `init_tracing()` — configure tracing subscriber with env filter
//!   and optional JSON output (shared across all 4 services)
//! - Common middleware configuration helpers
//!
//! See #47 for migration details.
