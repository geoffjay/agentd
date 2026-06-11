//! Domain types for the agentd-memory service.
//!
//! This module re-exports the complete public type surface used by the
//! memory service API, storage layer, and CLI client.
//!
//! # Core types
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Memory`] | A single memory record |
//! | [`MemoryType`] | Semantic category (`question`, `request`, `information`) |
//! | [`VisibilityLevel`] | Access-control tier (`public`, `shared`, `private`) |
//!
//! # Request / response types
//!
//! | Type | Endpoint |
//! |------|----------|
//! | [`CreateMemoryRequest`] | `POST /memories` |
//! | [`SearchRequest`] | `POST /memories/search` |
//! | [`UpdateVisibilityRequest`] | `PUT /memories/:id/visibility` |
//! | [`SearchResponse`] | Search results envelope |
//! | [`DeleteResponse`] | Delete confirmation |

mod memory;
mod memory_type;
mod requests;
mod visibility;

pub use memory::Memory;
pub use memory_type::MemoryType;
pub use requests::{
    CreateMemoryRequest, DeleteResponse, SearchRequest, SearchResponse, UpdateVisibilityRequest,
};
pub use visibility::VisibilityLevel;
