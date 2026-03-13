//! Shared types for the agentd-memory service.
//!
//! This module defines the core data types used throughout the memory service,
//! including memory records, visibility controls, and API request/response types.
//! Full type definitions for memory entries, embeddings, and semantic search will
//! be added in subsequent issues.

use serde::{Deserialize, Serialize};

/// Placeholder type representing a memory record.
///
/// Full fields (content, embedding, visibility, tags, source, etc.) will be
/// defined when the types issue is addressed.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// Unique identifier for this memory record.
    pub id: uuid::Uuid,
    /// Human-readable content of the memory.
    pub content: String,
    /// When this record was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
