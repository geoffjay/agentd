//! HTTP client for the agentd-knowledge service.
#![allow(dead_code)]
//!
//! Populated fully in KB-4.

use anyhow::{Context, Result};
use std::env;

/// HTTP client for the agentd-knowledge REST API.
#[derive(Debug, Clone)]
pub struct KnowledgeClient {
    base_url: String,
    client: reqwest::Client,
}

impl KnowledgeClient {
    /// Create a client pointing at `base_url`.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), client: reqwest::Client::new() }
    }

    /// Create a client from environment / config defaults.
    ///
    /// Reads `AGENTD_KNOWLEDGE_SERVICE_URL`, falling back to the shared config
    /// port (default `http://localhost:17011`).
    pub fn from_env() -> Self {
        let url = env::var("AGENTD_KNOWLEDGE_SERVICE_URL").unwrap_or_else(|_| {
            let cfg = agentd_common::config::load().unwrap_or_default();
            format!("http://localhost:{}", cfg.services.knowledge.port)
        });
        Self::new(url)
    }

    /// Check service health.
    pub async fn health(&self) -> Result<serde_json::Value> {
        let url = format!("{}/health", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .context("health request failed")?
            .json()
            .await
            .context("failed to parse health response")
    }
}
