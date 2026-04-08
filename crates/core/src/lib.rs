//! Core service library.
//!
//! The core service is the central authentication and API gateway for agentd.
//! This crate exposes its HTTP router so it can be tested without spawning the
//! full binary.

pub mod api;
