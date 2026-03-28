//! HTTP client wrapper for agentd services.
//!
//! Provides a thin `AgentdClient` that holds a `reqwest::Client` and the
//! base URLs for each agentd service. Tool implementations use this to make
//! API calls without constructing ad-hoc clients.

use crate::config::AgentdMcpConfig;
use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;

/// Shared HTTP client for all agentd service calls.
#[derive(Debug, Clone)]
pub struct AgentdClient {
    pub(crate) inner: Client,
    config: Arc<AgentdMcpConfig>,
}

impl AgentdClient {
    /// Create a new client from the given configuration.
    pub fn new(config: Arc<AgentdMcpConfig>) -> Self {
        Self { inner: Client::new(), config }
    }

    /// Returns the base URL for the orchestrator service.
    pub fn orchestrator_url(&self) -> &str {
        &self.config.orchestrator_url
    }

    /// Returns the base URL for the communicate service.
    pub fn communicate_url(&self) -> &str {
        &self.config.communicate_url
    }

    /// Returns the base URL for the memory service.
    pub fn memory_url(&self) -> &str {
        &self.config.memory_url
    }

    /// Returns the base URL for the notify service.
    pub fn notify_url(&self) -> &str {
        &self.config.notify_url
    }

    /// Returns the base URL for the ask service.
    pub fn ask_url(&self) -> &str {
        &self.config.ask_url
    }

    /// Returns the base URL for the wrap service.
    pub fn wrap_url(&self) -> &str {
        &self.config.wrap_url
    }

    /// Returns the base URL for the monitor service.
    pub fn monitor_url(&self) -> &str {
        &self.config.monitor_url
    }

    /// Perform a GET request against a service URL and deserialize the JSON response.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self.inner.get(url).send().await?.error_for_status()?;
        Ok(resp.json::<T>().await?)
    }

    /// Perform a POST request with a JSON body and deserialize the JSON response.
    #[allow(dead_code)]
    pub async fn post<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.inner.post(url).json(body).send().await?.error_for_status()?;
        Ok(resp.json::<T>().await?)
    }
}
