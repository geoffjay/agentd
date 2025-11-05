//! Core notification types and data structures.
//!
//! This module re-exports all types from the [`crate::types`] module for backward compatibility.
//! New code should prefer importing from `notify::types::*` directly.

// Re-export everything from types for backward compatibility
#[allow(unused_imports)]
pub use crate::types::*;
