//! HTTP client for the monitor service.
//!
//! Provides a typed client for interacting with a running `agentd-monitor` instance.
//!
//! # Examples
//!
//! ```no_run
//! use monitor::client::MonitorClient;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let client = MonitorClient::new("http://localhost:17003".to_string());
//!     let health = client.health().await?;
//!     println!("Status: {}", health.status);
//!     Ok(())
//! }
//! ```

use crate::types::{CollectResponse, HealthResponse, SystemMetrics, SystemStatus};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::time::Duration;

/// Typed HTTP client for the monitor service.
///
/// Wraps `reqwest::Client` with a 10-second timeout and provides methods for
/// each API endpoint.
#[derive(Clone)]
pub struct MonitorClient {
    base_url: String,
    http: Client,
}

impl MonitorClient {
    /// Create a new client pointing at `base_url` (e.g. `"http://localhost:17003"`).
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

    /// `GET /metrics` — fetch the latest system metrics snapshot.
    pub async fn get_metrics(&self) -> Result<SystemMetrics> {
        let url = format!("{}/metrics", self.base_url);
        let resp = self.http.get(&url).send().await.with_context(|| format!("GET {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("GET /metrics failed: HTTP {} — {}", status, body));
        }

        resp.json::<SystemMetrics>().await.context("Failed to parse metrics response")
    }

    /// `POST /collect` — trigger an immediate collection and return the snapshot.
    pub async fn collect(&self) -> Result<CollectResponse> {
        let url = format!("{}/collect", self.base_url);
        let resp = self.http.post(&url).send().await.with_context(|| format!("POST {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("POST /collect failed: HTTP {} — {}", status, body));
        }

        resp.json::<CollectResponse>().await.context("Failed to parse collect response")
    }

    /// `GET /history` — fetch all retained metric snapshots.
    pub async fn get_history(&self) -> Result<Vec<SystemMetrics>> {
        let url = format!("{}/history", self.base_url);
        let resp = self.http.get(&url).send().await.with_context(|| format!("GET {url}"))?;

        if !resp.status().is_success() {
            return Err(anyhow!("GET /history failed: HTTP {}", resp.status()));
        }

        resp.json::<Vec<SystemMetrics>>().await.context("Failed to parse history response")
    }

    /// `GET /status` — fetch the current health assessment.
    pub async fn get_status(&self) -> Result<SystemStatus> {
        let url = format!("{}/status", self.base_url);
        let resp = self.http.get(&url).send().await.with_context(|| format!("GET {url}"))?;

        if !resp.status().is_success() {
            return Err(anyhow!("GET /status failed: HTTP {}", resp.status()));
        }

        resp.json::<SystemStatus>().await.context("Failed to parse status response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = MonitorClient::new("http://localhost:17003".to_string());
        assert_eq!(client.base_url, "http://localhost:17003");
    }

    #[test]
    fn test_client_clone() {
        let client = MonitorClient::new("http://localhost:17003".to_string());
        let cloned = client.clone();
        assert_eq!(client.base_url, cloned.base_url);
    }

    #[tokio::test]
    async fn test_health_returns_error_on_connection_refused() {
        // Use a port that should not have anything listening
        let client = MonitorClient::new("http://127.0.0.1:19876".to_string());
        let result = client.health().await;
        assert!(result.is_err(), "Should fail when service is not running");
    }

    #[tokio::test]
    async fn test_collect_returns_error_on_connection_refused() {
        let client = MonitorClient::new("http://127.0.0.1:19876".to_string());
        let result = client.collect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_metrics_returns_error_on_connection_refused() {
        let client = MonitorClient::new("http://127.0.0.1:19876".to_string());
        let result = client.get_metrics().await;
        assert!(result.is_err());
    }
}
