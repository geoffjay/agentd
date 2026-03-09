//! HTTP client for the hook service.
//!
//! Provides a typed client for interacting with a running `agentd-hook` instance.
//! Used by shell integrations and the CLI to submit events and query history.
//!
//! # Examples
//!
//! ```no_run
//! use hook::client::HookClient;
//! use hook::types::{HookEvent, HookKind};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = HookClient::new("http://localhost:17002".to_string());
//!     let event = HookEvent {
//!         kind: HookKind::Shell,
//!         command: "cargo build".to_string(),
//!         exit_code: 0,
//!         duration_ms: 1200,
//!         output: None,
//!         metadata: Default::default(),
//!     };
//!     let resp = client.submit_event(event).await?;
//!     println!("Event ID: {}", resp.event_id);
//!     Ok(())
//! }
//! ```

use crate::types::{EventResponse, HealthResponse, HookEvent, RecordedEvent};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::time::Duration;

/// Typed HTTP client for the hook service.
#[derive(Clone)]
pub struct HookClient {
    base_url: String,
    http: Client,
}

impl HookClient {
    /// Create a new client pointing at `base_url` (e.g. `"http://localhost:17002"`).
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest Client");
        Self { base_url, http }
    }

    /// `GET /health` — check whether the service is reachable.
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let resp = self.http.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("Health check failed: HTTP {}", resp.status()));
        }
        resp.json::<HealthResponse>().await.context("Failed to parse health response")
    }

    /// `POST /events` — submit a shell or git hook event.
    pub async fn submit_event(&self, event: HookEvent) -> Result<EventResponse> {
        let url = format!("{}/events", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&event)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("POST /events failed: HTTP {} — {}", status, body));
        }
        resp.json::<EventResponse>().await.context("Failed to parse event response")
    }

    /// `GET /events` — list recent events (newest first, default limit 50).
    pub async fn list_events(&self, limit: Option<usize>) -> Result<Vec<RecordedEvent>> {
        let limit = limit.unwrap_or(50);
        let url = format!("{}/events?limit={}", self.base_url, limit);
        let resp = self.http.get(&url).send().await.with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            return Err(anyhow!("GET /events failed: HTTP {}", resp.status()));
        }
        resp.json::<Vec<RecordedEvent>>().await.context("Failed to parse events list")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HookClient::new("http://localhost:17002".to_string());
        assert_eq!(client.base_url, "http://localhost:17002");
    }

    #[test]
    fn test_client_clone() {
        let client = HookClient::new("http://localhost:17002".to_string());
        let cloned = client.clone();
        assert_eq!(client.base_url, cloned.base_url);
    }

    #[tokio::test]
    async fn test_health_returns_error_when_unreachable() {
        let client = HookClient::new("http://127.0.0.1:19877".to_string());
        assert!(client.health().await.is_err());
    }

    #[tokio::test]
    async fn test_submit_event_returns_error_when_unreachable() {
        let client = HookClient::new("http://127.0.0.1:19877".to_string());
        let event = HookEvent {
            kind: crate::types::HookKind::Shell,
            command: "test".to_string(),
            exit_code: 0,
            duration_ms: 0,
            output: None,
            metadata: Default::default(),
        };
        assert!(client.submit_event(event).await.is_err());
    }
}
