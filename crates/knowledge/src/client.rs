//! HTTP client for the agentd-knowledge service.
//!
//! [`KnowledgeClient`] covers all REST endpoints and is reusable from the
//! CLI and (later) the orchestrator. It is intentionally thin — no caching,
//! no retry, just a typed facade over `reqwest`.

use anyhow::{Context, Result};
use serde_json::Value;
use std::env;

use crate::types::{
    CreateDocumentRequest, Document, DocumentContent, PaginatedResponse, TreeNode,
    UpdateDocumentRequest,
};

/// HTTP client for the agentd-knowledge REST API.
#[derive(Debug, Clone)]
pub struct KnowledgeClient {
    base_url: String,
    client: reqwest::Client,
    token: Option<String>,
}

impl KnowledgeClient {
    /// Create a client pointing at `base_url`.
    ///
    /// When routed through the core gateway, `base_url` should be
    /// `{AGENTD_CORE_SERVICE_URL}/api/v1/knowledge` and a bearer token should
    /// be attached via [`with_token`](Self::with_token).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), client: reqwest::Client::new(), token: None }
    }

    /// Attach a bearer token to all requests made by this client.
    ///
    /// The core gateway validates the token, resolves the active organization,
    /// and injects `X-Tenant-ID` before forwarding to the knowledge service.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Create a client from environment / config defaults.
    ///
    /// Reads `AGENTD_KNOWLEDGE_SERVICE_URL`, falling back to the shared config
    /// port (default `http://localhost:17011`). This bypasses the core gateway
    /// and is intended for trusted/local callers; gateway-routed callers should
    /// use [`new`](Self::new) with a gateway URL plus [`with_token`](Self::with_token).
    pub fn from_env() -> Self {
        let url = env::var("AGENTD_KNOWLEDGE_SERVICE_URL").unwrap_or_else(|_| {
            let cfg = agentd_common::config::load().unwrap_or_default();
            format!("http://localhost:{}", cfg.services.knowledge.port)
        });
        Self::new(url)
    }

    /// Apply the bearer token (if set) to a request builder.
    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.token {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }

    // -----------------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------------

    /// Check service health.
    pub async fn health(&self) -> Result<Value> {
        let url = format!("{}/health", self.base_url);
        let res = self.auth(self.client.get(&url)).send().await.context("health request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse health response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("health failed ({status}): {body}");
        }
    }

    // -----------------------------------------------------------------------
    // Documents — collection
    // -----------------------------------------------------------------------

    /// List documents for `project_id` with optional prefix filter and pagination.
    pub async fn list_documents(
        &self,
        project_id: &str,
        prefix: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<PaginatedResponse<Document>> {
        let mut url = format!("{}/projects/{project_id}/documents", self.base_url);
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(p) = prefix {
            params.push(("prefix", p.to_string()));
        }
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }
        if let Some(o) = offset {
            params.push(("offset", o.to_string()));
        }
        if !params.is_empty() {
            use std::fmt::Write;
            url.push('?');
            for (i, (k, v)) in params.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                let _ = write!(url, "{k}={}", urlencoding::encode(v));
            }
        }
        // NOTE: project_id and doc_id are UUIDs in practice; path-segment
        // encoding is not applied here but would be required for arbitrary IDs.
        let res = self
            .auth(self.client.get(&url))
            .send()
            .await
            .context("list_documents request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse list_documents response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("list_documents failed ({status}): {body}");
        }
    }

    /// Create a new document.
    pub async fn create_document(
        &self,
        project_id: &str,
        req: CreateDocumentRequest,
    ) -> Result<Document> {
        let url = format!("{}/projects/{project_id}/documents", self.base_url);
        let res = self
            .auth(self.client.post(&url))
            .json(&req)
            .send()
            .await
            .context("create_document request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse create_document response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("create_document failed ({status}): {body}");
        }
    }

    /// Bulk-delete all documents for `project_id` (project cleanup).
    pub async fn bulk_delete_documents(&self, project_id: &str) -> Result<()> {
        let url = format!("{}/projects/{project_id}/documents", self.base_url);
        let res = self
            .auth(self.client.delete(&url))
            .send()
            .await
            .context("bulk_delete_documents request failed")?;
        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("bulk_delete_documents failed ({status}): {body}");
        }
    }

    // -----------------------------------------------------------------------
    // Documents — instance
    // -----------------------------------------------------------------------

    /// Get document metadata by ID.
    pub async fn get_document(&self, project_id: &str, doc_id: &str) -> Result<Document> {
        let url = format!("{}/projects/{project_id}/documents/{doc_id}", self.base_url);
        let res =
            self.auth(self.client.get(&url)).send().await.context("get_document request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse get_document response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("get_document failed ({status}): {body}");
        }
    }

    /// Get document metadata + content by ID.
    pub async fn get_document_content(
        &self,
        project_id: &str,
        doc_id: &str,
    ) -> Result<DocumentContent> {
        let url = format!("{}/projects/{project_id}/documents/{doc_id}/content", self.base_url);
        let res = self
            .auth(self.client.get(&url))
            .send()
            .await
            .context("get_document_content request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse get_document_content response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("get_document_content failed ({status}): {body}");
        }
    }

    /// Update a document.
    pub async fn update_document(
        &self,
        project_id: &str,
        doc_id: &str,
        req: UpdateDocumentRequest,
    ) -> Result<Document> {
        let url = format!("{}/projects/{project_id}/documents/{doc_id}", self.base_url);
        let res = self
            .auth(self.client.put(&url))
            .json(&req)
            .send()
            .await
            .context("update_document request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse update_document response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("update_document failed ({status}): {body}");
        }
    }

    /// Delete a document by ID.
    pub async fn delete_document(&self, project_id: &str, doc_id: &str) -> Result<()> {
        let url = format!("{}/projects/{project_id}/documents/{doc_id}", self.base_url);
        let res = self
            .auth(self.client.delete(&url))
            .send()
            .await
            .context("delete_document request failed")?;
        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("delete_document failed ({status}): {body}");
        }
    }

    // -----------------------------------------------------------------------
    // Tree
    // -----------------------------------------------------------------------

    /// Get the virtual folder/file tree for `project_id`.
    pub async fn get_tree(&self, project_id: &str) -> Result<Vec<TreeNode>> {
        let url = format!("{}/projects/{project_id}/tree", self.base_url);
        let res =
            self.auth(self.client.get(&url)).send().await.context("get_tree request failed")?;
        if res.status().is_success() {
            res.json().await.context("failed to parse get_tree response")
        } else {
            let status = res.status();
            let body: Value = res.json().await.unwrap_or(Value::Null);
            anyhow::bail!("get_tree failed ({status}): {body}");
        }
    }
}

// ---------------------------------------------------------------------------
// URL encoding helper
// ---------------------------------------------------------------------------

mod urlencoding {
    /// Percent-encode a string for use in a query parameter value.
    pub fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                out.push(b as char);
            } else {
                use std::fmt::Write;
                let _ = write!(out, "%{b:02X}");
            }
        }
        out
    }
}
