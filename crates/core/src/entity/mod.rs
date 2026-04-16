//! SeaORM entity definitions for the core crate.
//!
//! Four entities map to the core service tables:
//!
//! - [`user`] → `users` table
//! - [`organization`] → `organizations` table
//! - [`membership`] → `memberships` table
//! - [`session`] → `sessions` table

pub mod membership;
pub mod organization;
pub mod session;
pub mod user;
