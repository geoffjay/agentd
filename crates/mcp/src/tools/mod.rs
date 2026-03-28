//! MCP tool implementations for agentd services.
//!
//! Tools are grouped by the agentd service they interact with.
//!
//! # Tool Modules
//!
//! - `diagnostic` — cross-service diagnostic and connectivity tools (issue #256)
//!
//! # Planned Tool Modules
//!
//! - `agents`    — list/inspect agents (issue #250)
//! - `rooms`     — list rooms, send/read messages (issue #251)
//! - `memory`    — search and store memories (issue #252)
//! - `notify`    — create and respond to notifications (issue #253)
//! - `ask`       — trigger and answer approval requests (issue #254)
//! - `wrap`      — manage wrap configurations (issue #255)
//! - `workflow`  — dispatch and monitor workflows (issue #256)

pub mod approvals;
pub mod diagnostic;
pub mod health;
pub mod lifecycle;
