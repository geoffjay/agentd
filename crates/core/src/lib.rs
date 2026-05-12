//! Core service library.
//!
//! The core service is the central authentication and API gateway for agentd.
//! This crate exposes its HTTP router so it can be tested without spawning the
//! full binary.

pub mod api;
pub mod config;
pub mod entity;
pub mod membership_storage;
pub mod middleware;
pub mod migration;
pub mod organization_storage;
pub mod proxy;
pub mod session_storage;
pub mod storage;
pub mod user_storage;
