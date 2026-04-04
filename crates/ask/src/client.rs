//! HTTP client for interacting with the ask service.
//!
//! This module provides a strongly-typed client for making requests to the
//! ask service REST API. Full Q&A client methods are implemented in issue #925.

use crate::types::HealthResponse;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

/// Client for the ask service REST API.
#[derive(Clone)]
pub struct AskClient {
    client: reqwest::Client,
    base_url: String,
}

impl AskClient {
    /// Create a new ask service client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Check the health of the ask service.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = AskClient::new("http://localhost:7001");
        assert_eq!(client.base_url, "http://localhost:7001");
    }

    #[test]
    fn test_client_clone() {
        let client1 = AskClient::new("http://localhost:7001");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
