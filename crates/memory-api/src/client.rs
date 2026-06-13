//! HTTP client for interacting with the agentd-memory service.
//!
//! Provides a strongly-typed client for making requests to the memory service
//! REST API. Handles serialization, deserialization, and query parameter
//! construction.
//!
//! # Examples
//!
//! ```no_run
//! use memory_api::client::MemoryClient;
//! use memory_api::types::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = MemoryClient::new("http://localhost:17008");
//!
//! // Create a memory
//! let request = CreateMemoryRequest {
//!     content: "Paris is the capital of France.".to_string(),
//!     created_by: "agent-1".to_string(),
//!     ..Default::default()
//! };
//! let memory = client.create_memory(&request).await?;
//!
//! // Semantic search
//! let results = client.search_memories(&SearchRequest {
//!     query: "capital of France".to_string(),
//!     ..Default::default()
//! }).await?;
//! println!("Found {} results", results.total);
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::types::{
    CreateMemoryRequest, DeleteResponse, Memory, SearchRequest, SearchResponse,
    UpdateVisibilityRequest,
};
use agentd_common::types::PaginatedResponse;

/// Client for the agentd-memory service REST API.
///
/// Provides strongly-typed methods for all memory operations including
/// creating, listing, searching, and deleting memories.
///
/// # Examples
///
/// ```
/// use memory_api::client::MemoryClient;
///
/// let client = MemoryClient::new("http://localhost:17008");
/// ```
#[derive(Clone)]
pub struct MemoryClient {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl MemoryClient {
    /// Create a new memory service client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the memory service (e.g., `"http://localhost:17008"`)
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into(), token: None }
    }

    /// Attach a bearer token to all requests made by this client.
    ///
    /// Returns `self` for method chaining.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// `POST /memories` — create a new memory record.
    pub async fn create_memory(&self, request: &CreateMemoryRequest) -> Result<Memory> {
        self.post("/memories", request).await
    }

    /// `GET /memories/:id` — retrieve a single memory by ID.
    pub async fn get_memory(&self, id: &str) -> Result<Memory> {
        self.get(&format!("/memories/{id}")).await
    }

    /// `GET /memories` — list memories with optional filters.
    ///
    /// # Arguments
    ///
    /// * `memory_type` — filter by type (`information`, `question`, `request`)
    /// * `tag` — filter by tag (comma-separated for multiple)
    /// * `created_by` — filter by creator identity
    /// * `visibility` — filter by visibility level
    /// * `limit` — max items per page
    /// * `offset` — pagination offset
    pub async fn list_memories(
        &self,
        memory_type: Option<&str>,
        tag: Option<&str>,
        created_by: Option<&str>,
        visibility: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<PaginatedResponse<Memory>> {
        let url = format!("{}/memories", self.base_url);
        let mut request = self.client.get(&url);

        if let Some(tok) = &self.token {
            request = request.bearer_auth(tok);
        }
        if let Some(t) = memory_type {
            request = request.query(&[("type", t)]);
        }
        if let Some(t) = tag {
            request = request.query(&[("tag", t)]);
        }
        if let Some(c) = created_by {
            request = request.query(&[("created_by", c)]);
        }
        if let Some(v) = visibility {
            request = request.query(&[("visibility", v)]);
        }
        if let Some(l) = limit {
            request = request.query(&[("limit", &l.to_string())]);
        }
        if let Some(o) = offset {
            request = request.query(&[("offset", &o.to_string())]);
        }

        let response = request.send().await.context(format!("Failed to GET {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    /// `DELETE /memories/:id` — delete a memory.
    pub async fn delete_memory(&self, id: &str) -> Result<DeleteResponse> {
        self.delete_json(&format!("/memories/{id}")).await
    }

    /// `POST /memories/search` — semantic similarity search.
    pub async fn search_memories(&self, request: &SearchRequest) -> Result<SearchResponse> {
        self.post("/memories/search", request).await
    }

    /// `PUT /memories/:id/visibility` — update visibility and share list.
    pub async fn update_visibility(
        &self,
        id: &str,
        request: &UpdateVisibilityRequest,
    ) -> Result<Memory> {
        self.put(&format!("/memories/{id}/visibility"), request).await
    }

    /// `GET /health` — service health check.
    pub async fn health(&self) -> Result<serde_json::Value> {
        self.get("/health").await
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let response = req.send().await.context(format!("Failed to GET {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url).json(body);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let response = req.send().await.context(format!("Failed to POST {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn put<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.put(&url).json(body);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let response = req.send().await.context(format!("Failed to PUT {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.delete(&url);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let response = req.send().await.context(format!("Failed to DELETE {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = MemoryClient::new("http://localhost:17008");
        assert_eq!(client.base_url, "http://localhost:17008");
    }

    #[test]
    fn test_client_creation_with_string() {
        let url = String::from("http://localhost:17008");
        let client = MemoryClient::new(url);
        assert_eq!(client.base_url, "http://localhost:17008");
    }

    #[test]
    fn test_client_clone() {
        let client1 = MemoryClient::new("http://localhost:17008");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
