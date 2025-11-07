//! HTTP client for interacting with the wrap service.
//!
//! This module provides a strongly-typed client for making requests to the
//! wrap service REST API. It handles serialization/deserialization and provides
//! ergonomic methods for all wrap operations.
//!
//! # Examples
//!
//! ## Creating a client and checking health
//!
//! ```no_run
//! use wrap::client::WrapClient;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = WrapClient::new("http://localhost:7005");
//! client.health().await?;
//! println!("Service is healthy");
//! # Ok(())
//! # }
//! ```
//!
//! ## Launching an agent
//!
//! ```no_run
//! use wrap::client::WrapClient;
//! use wrap::types::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = WrapClient::new("http://localhost:7005");
//!
//! let request = LaunchRequest {
//!     project_name: "my-project".to_string(),
//!     project_path: "/path/to/project".to_string(),
//!     agent_type: "claude-code".to_string(),
//!     model_provider: "anthropic".to_string(),
//!     model_name: "claude-sonnet-4.5".to_string(),
//!     layout: None,
//! };
//!
//! let response = client.launch(&request).await?;
//! if let Some(session_name) = response.session_name {
//!     println!("Launched agent in session: {}", session_name);
//! }
//! # Ok(())
//! # }
//! ```

use crate::types::*;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Client for the wrap service REST API.
///
/// Provides strongly-typed methods for all wrap operations including
/// launching agents and checking service health.
///
/// # Examples
///
/// ```
/// use wrap::client::WrapClient;
///
/// let client = WrapClient::new("http://localhost:7005");
/// ```
#[derive(Clone)]
pub struct WrapClient {
    client: reqwest::Client,
    base_url: String,
}

impl WrapClient {
    /// Create a new wrap service client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the wrap service (e.g., "http://localhost:7005")
    ///
    /// # Examples
    ///
    /// ```
    /// use wrap::client::WrapClient;
    ///
    /// let client = WrapClient::new("http://localhost:7005");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Launch an agent in a tmux session.
    ///
    /// Creates a new tmux session and starts the specified agent with the
    /// given configuration. The agent will run in the background in the
    /// tmux session.
    ///
    /// # Arguments
    ///
    /// * `request` - The launch configuration request
    ///
    /// # Returns
    ///
    /// Returns a `LaunchResponse` containing the session information and
    /// initial status.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use wrap::client::WrapClient;
    /// # use wrap::types::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = WrapClient::new("http://localhost:7005");
    ///
    /// let request = LaunchRequest {
    ///     project_name: "my-project".to_string(),
    ///     project_path: "/path/to/project".to_string(),
    ///     agent_type: "claude-code".to_string(),
    ///     model_provider: "anthropic".to_string(),
    ///     model_name: "claude-sonnet-4.5".to_string(),
    ///     layout: None,
    /// };
    ///
    /// let response = client.launch(&request).await?;
    /// if let Some(session_name) = response.session_name {
    ///     println!("Launched session: {}", session_name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn launch(&self, request: &LaunchRequest) -> Result<LaunchResponse> {
        self.post("/launch", request).await
    }

    /// Check the health of the wrap service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use wrap::client::WrapClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = WrapClient::new("http://localhost:7005");
    /// client.health().await?;
    /// println!("Service is healthy");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health(&self) -> Result<()> {
        self.get::<HealthResponse>("/health").await?;
        Ok(())
    }

    // Internal helper methods

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

    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;

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
        let client = WrapClient::new("http://localhost:7005");
        assert_eq!(client.base_url, "http://localhost:7005");
    }

    #[test]
    fn test_client_creation_with_string() {
        let url = String::from("http://localhost:7005");
        let client = WrapClient::new(url);
        assert_eq!(client.base_url, "http://localhost:7005");
    }

    #[test]
    fn test_client_clone() {
        let client1 = WrapClient::new("http://localhost:7005");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
