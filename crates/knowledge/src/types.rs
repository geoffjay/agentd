//! Shared domain types for the agentd-knowledge service.
#![allow(dead_code)]
//!
//! DTOs and request/response structures used by the REST API, client, and storage layers.

use serde::{Deserialize, Serialize};

/// Metadata for a knowledge document (no body content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// UUID of the document.
    pub id: String,
    /// UUID of the owning project.
    pub project_id: String,
    /// Relative path within the project (includes `.md` extension).
    pub rel_path: String,
    /// Document title (defaults to the filename without extension).
    pub title: String,
    /// Size of the document body in bytes.
    pub size_bytes: i64,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// RFC3339 last-update timestamp.
    pub updated_at: String,
    /// Optional organization UUID for tenant scoping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
}

/// Document metadata plus markdown body content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    #[serde(flatten)]
    pub document: Document,
    /// Raw markdown body.
    pub content: String,
}

/// Request body for creating a new document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    /// Relative path (must end in `.md`; no `..`; no absolute paths).
    pub rel_path: String,
    /// Optional title override. Defaults to the filename stem.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Initial markdown content.
    #[serde(default)]
    pub content: String,
}

/// Request body for updating a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentRequest {
    /// New markdown content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// New title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optimistic concurrency: reject if `updated_at` in DB differs from this.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_updated_at: Option<String>,
}

/// A node in the virtual document tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TreeNode {
    /// A folder (implied by rel_path prefixes).
    Folder { name: String, path: String, children: Vec<TreeNode> },
    /// A document leaf.
    File { name: String, path: String, doc_id: String },
}

/// Paginated list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

/// Result of a `doctor` reconciliation pass for a single project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorReport {
    /// DB rows whose markdown file is absent from disk.
    pub missing_files: Vec<String>,
    /// Disk files that have no corresponding DB row.
    pub orphaned_files: Vec<String>,
    /// Number of issues automatically repaired (set when `fix = true`).
    pub fixed: u32,
}
