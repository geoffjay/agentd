//! Error types for the agentd-knowledge service.
#![allow(dead_code)]

use thiserror::Error;

/// Errors that can occur within the knowledge service.
#[derive(Debug, Error)]
pub enum KnowledgeError {
    /// A document already exists at the given path for this project.
    #[error("conflict: {0}")]
    Conflict(String),

    /// The requested document does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// The provided path is invalid (e.g., contains `..`, not `.md`, too deep).
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// An internal error (database, filesystem, etc.)
    #[error("internal error: {0}")]
    Other(#[from] anyhow::Error),
}
