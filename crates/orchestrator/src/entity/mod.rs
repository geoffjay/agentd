//! SeaORM entity definitions for the orchestrator crate.
//!
//! Three entities map to the three SQLite tables:
//!
//! - [`agent`] → `agents` table
//! - [`workflow`] → `workflows` table
//! - [`dispatch`] → `dispatch_log` table

pub mod agent;
pub mod dispatch;
pub mod workflow;
