//! Shared SQLite storage utilities.
//!
//! This module will contain:
//! - `get_db_path(service_name)` — resolve XDG-compliant database file paths
//! - `create_pool(path)` — create and configure a SQLite connection pool
//! - Test helpers for creating temporary in-memory databases
//!
//! See #46 for migration details.
