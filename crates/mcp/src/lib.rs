//! agentd-mcp library — exposes internal modules for integration testing.
//!
//! The binary entry point is `main.rs`. This lib re-exports the key modules
//! so that integration tests in `tests/` can access `AgentdClient`,
//! `AgentdMcpConfig`, and the tool implementation functions.

pub mod client;
pub mod config;
pub mod server;
pub mod tools;
