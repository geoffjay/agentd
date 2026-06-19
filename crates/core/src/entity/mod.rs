//! SeaORM entity definitions for the core crate.
//!
//! Five entities map to the core service tables:
//!
//! - [`user`] → `users` table
//! - [`organization`] → `organizations` table
//! - [`membership`] → `memberships` table
//! - [`session`] → `sessions` table
//! - [`project`] → `projects` table

pub mod membership;
pub mod organization;
pub mod project;
pub mod session;
pub mod user;
