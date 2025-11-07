//! HTTP client for interacting with the ask service.
//!
//! This module provides a strongly-typed client for making requests to the
//! ask service REST API. It handles serialization/deserialization and provides
//! ergonomic methods for all ask service operations.
//!
//! # Examples
//!
//! ## Creating a client and triggering checks
//!
//! ```no_run
//! use ask::client::AskClient;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = AskClient::new("http://localhost:7001");
//! let response = client.trigger_checks().await?;
//! println!("Ran {} checks", response.checks_run.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Answering a question
//!
//! ```no_run
//! use ask::client::AskClient;
//! use ask::types::AnswerRequest;
//! use uuid::Uuid;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = AskClient::new("http://localhost:7001");
//!
//! let request = AnswerRequest {
//!     question_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?,
//!     answer: "yes".to_string(),
//! };
//!
//! let response = client.answer_question(&request).await?;
//! println!("{}", response.message);
//! # Ok(())
//! # }
//! ```

use crate::types::*;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Client for the ask service REST API.
///
/// Provides strongly-typed methods for all ask service operations including
/// triggering checks and answering questions.
///
/// # Examples
///
/// ```
/// use ask::client::AskClient;
///
/// let client = AskClient::new("http://localhost:7001");
/// ```
#[derive(Clone)]
pub struct AskClient {
    client: reqwest::Client,
    base_url: String,
}

impl AskClient {
    /// Create a new ask service client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the ask service (e.g., "http://localhost:7001")
    ///
    /// # Examples
    ///
    /// ```
    /// use ask::client::AskClient;
    ///
    /// let client = AskClient::new("http://localhost:7001");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Trigger environment checks.
    ///
    /// Runs all configured checks (e.g., tmux session check) and creates
    /// notifications for any conditions that require user attention.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ask::client::AskClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = AskClient::new("http://localhost:7001");
    /// let response = client.trigger_checks().await?;
    /// println!("Ran {} checks", response.checks_run.len());
    /// println!("Created {} notifications", response.notifications_sent.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn trigger_checks(&self) -> Result<TriggerResponse> {
        self.post("/trigger", &()).await
    }

    /// Submit an answer to a question.
    ///
    /// # Arguments
    ///
    /// * `request` - The answer request containing the question ID and answer text
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ask::client::AskClient;
    /// # use ask::types::AnswerRequest;
    /// # use uuid::Uuid;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = AskClient::new("http://localhost:7001");
    ///
    /// let request = AnswerRequest {
    ///     question_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?,
    ///     answer: "yes".to_string(),
    /// };
    ///
    /// let response = client.answer_question(&request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn answer_question(&self, request: &AnswerRequest) -> Result<AnswerResponse> {
        self.post("/answer", request).await
    }

    /// Check the health of the ask service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ask::client::AskClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = AskClient::new("http://localhost:7001");
    /// let health = client.health().await?;
    /// println!("Service: {}", health.service);
    /// println!("Status: {}", health.status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
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
        let client = AskClient::new("http://localhost:7001");
        assert_eq!(client.base_url, "http://localhost:7001");
    }

    #[test]
    fn test_client_creation_with_string() {
        let url = String::from("http://localhost:7001");
        let client = AskClient::new(url);
        assert_eq!(client.base_url, "http://localhost:7001");
    }

    #[test]
    fn test_client_clone() {
        let client1 = AskClient::new("http://localhost:7001");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
