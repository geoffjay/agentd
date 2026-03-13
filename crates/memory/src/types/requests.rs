//! Request and response types for the memory service API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Memory, MemoryType, VisibilityLevel};

/// Request body for `POST /memories` — create a new memory record.
///
/// # Minimal example
///
/// ```json
/// { "content": "Paris is the capital of France.", "created_by": "agent-1" }
/// ```
///
/// # Full example
///
/// ```json
/// {
///   "content": "How do I reset my password?",
///   "type": "question",
///   "tags": ["auth", "help"],
///   "created_by": "user-42",
///   "references": ["mem_1_abc12345"],
///   "visibility": "shared",
///   "shared_with": ["agent-support"]
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateMemoryRequest {
    /// The natural-language content to store.
    pub content: String,

    /// Semantic category (defaults to `information`).
    #[serde(rename = "type", default)]
    pub memory_type: MemoryType,

    /// Free-form tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Identity of the actor creating this memory.
    pub created_by: String,

    /// IDs of related memories this record references.
    #[serde(default)]
    pub references: Vec<String>,

    /// Access control tier (defaults to `public`).
    #[serde(default)]
    pub visibility: VisibilityLevel,

    /// Actors to share with when `visibility` is `shared`.
    #[serde(default)]
    pub shared_with: Vec<String>,
}

/// Request body for `POST /memories/search` — semantic similarity search.
///
/// All filters are optional and are ANDed together.
///
/// # Example
///
/// ```json
/// {
///   "query": "password reset instructions",
///   "as_actor": "user-42",
///   "type": "question",
///   "tags": ["auth"],
///   "limit": 5
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Natural-language query string used for vector similarity search.
    pub query: String,

    /// Actor performing the search; used to filter by visibility.
    /// Anonymous searches only return `public` memories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_actor: Option<String>,

    /// Restrict results to a specific memory type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub memory_type: Option<MemoryType>,

    /// Restrict results to memories that have *at least one* of these tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Return only memories created on or after this timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<DateTime<Utc>>,

    /// Return only memories created on or before this timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<DateTime<Utc>>,

    /// Maximum number of results to return (defaults to 10).
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            as_actor: None,
            memory_type: None,
            tags: Vec::new(),
            from: None,
            to: None,
            limit: default_search_limit(),
        }
    }
}

fn default_search_limit() -> usize {
    10
}

/// Request body for `PUT /memories/:id/visibility`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateVisibilityRequest {
    /// New visibility level to apply.
    pub visibility: VisibilityLevel,

    /// Updated share list. Required when `visibility` is `shared`;
    /// ignored otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_with: Option<Vec<String>>,
}

/// Response envelope for search results from `POST /memories/search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Matching memory records, ordered by similarity score.
    pub memories: Vec<Memory>,

    /// Total number of matches (before applying `limit`).
    pub total: usize,
}

/// Response body for `DELETE /memories/:id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResponse {
    /// `true` if a record was actually removed; `false` if the ID was not found.
    pub deleted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── CreateMemoryRequest ────────────────────────────────────────────────

    #[test]
    fn test_create_request_required_fields_only() {
        let json = r#"{"content": "hello", "created_by": "agent-1"}"#;
        let req: CreateMemoryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.content, "hello");
        assert_eq!(req.created_by, "agent-1");
        assert_eq!(req.memory_type, MemoryType::Information);
        assert_eq!(req.visibility, VisibilityLevel::Public);
        assert!(req.tags.is_empty());
        assert!(req.references.is_empty());
        assert!(req.shared_with.is_empty());
    }

    #[test]
    fn test_create_request_full() {
        let json = r#"{
            "content": "How do I reset my password?",
            "type": "question",
            "tags": ["auth", "help"],
            "created_by": "user-42",
            "references": ["mem_1_abc12345"],
            "visibility": "shared",
            "shared_with": ["agent-support"]
        }"#;
        let req: CreateMemoryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.memory_type, MemoryType::Question);
        assert_eq!(req.tags, ["auth", "help"]);
        assert_eq!(req.references, ["mem_1_abc12345"]);
        assert_eq!(req.visibility, VisibilityLevel::Shared);
        assert_eq!(req.shared_with, ["agent-support"]);
    }

    // ── SearchRequest ──────────────────────────────────────────────────────

    #[test]
    fn test_search_request_default_limit() {
        let req = SearchRequest { query: "test".to_string(), ..Default::default() };
        assert_eq!(req.limit, 10);
    }

    #[test]
    fn test_search_request_optional_fields_absent() {
        let req = SearchRequest::default();
        assert!(req.as_actor.is_none());
        assert!(req.memory_type.is_none());
        assert!(req.tags.is_empty());
        assert!(req.from.is_none());
        assert!(req.to.is_none());
    }

    #[test]
    fn test_search_request_serialization() {
        let req = SearchRequest {
            query: "capital of France".to_string(),
            as_actor: Some("user-1".to_string()),
            memory_type: Some(MemoryType::Information),
            limit: 5,
            ..Default::default()
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"query\":\"capital of France\""));
        assert!(json.contains("\"as_actor\":\"user-1\""));
        assert!(json.contains("\"type\":\"information\""));
        assert!(json.contains("\"limit\":5"));
        // empty tags should be omitted
        assert!(!json.contains("\"tags\""));
    }

    #[test]
    fn test_search_request_tags_omitted_when_empty() {
        let req = SearchRequest { query: "q".to_string(), ..Default::default() };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"tags\""));
    }

    // ── UpdateVisibilityRequest ────────────────────────────────────────────

    #[test]
    fn test_update_visibility_request_roundtrip() {
        let req = UpdateVisibilityRequest {
            visibility: VisibilityLevel::Shared,
            shared_with: Some(vec!["user-a".to_string(), "user-b".to_string()]),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: UpdateVisibilityRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.visibility, VisibilityLevel::Shared);
        assert_eq!(parsed.shared_with.unwrap().len(), 2);
    }

    #[test]
    fn test_update_visibility_shared_with_omitted_when_none() {
        let req = UpdateVisibilityRequest { visibility: VisibilityLevel::Public, shared_with: None };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"shared_with\""));
    }

    // ── SearchResponse / DeleteResponse ───────────────────────────────────

    #[test]
    fn test_search_response_serialization() {
        let resp = SearchResponse { memories: vec![], total: 0 };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"memories\":[]"));
        assert!(json.contains("\"total\":0"));
    }

    #[test]
    fn test_delete_response_deleted_true() {
        let resp = DeleteResponse { deleted: true };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"deleted\":true"));
    }

    #[test]
    fn test_delete_response_deleted_false() {
        let resp = DeleteResponse { deleted: false };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"deleted\":false"));
    }
}
