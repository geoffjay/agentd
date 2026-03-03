//! HTTP client for the orchestrator service.
//!
//! Provides a typed client for communicating with the orchestrator REST API.
//! All methods are async and return strongly-typed response objects.
//!
//! # Examples
//!
//! ```rust
//! use orchestrator::client::OrchestratorClient;
//!
//! let client = OrchestratorClient::new("http://localhost:7006");
//! ```
//!
//! ```rust,no_run
//! # use orchestrator::client::OrchestratorClient;
//! # async fn example() -> anyhow::Result<()> {
//! let client = OrchestratorClient::new("http://localhost:7006");
//! let agents = client.list_agents(None).await?;
//! for agent in &agents.items {
//!     println!("{}: {}", agent.name, agent.status);
//! }
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::scheduler::types::{
    CreateWorkflowRequest, DispatchResponse, UpdateWorkflowRequest, WorkflowResponse,
};
use crate::types::{
    AgentResponse, ApprovalActionRequest, ApprovalStatus, CreateAgentRequest, HealthResponse,
    PaginatedResponse, PendingApproval, SendMessageRequest, SendMessageResponse, ToolPolicy,
};

/// Typed HTTP client for the orchestrator service.
///
/// Provides strongly-typed methods for all orchestrator REST API endpoints,
/// including agent management, workflow operations, and health checks.
///
/// # Examples
///
/// ```rust
/// use orchestrator::client::OrchestratorClient;
///
/// let client = OrchestratorClient::new("http://localhost:7006");
/// ```
#[derive(Clone)]
pub struct OrchestratorClient {
    client: reqwest::Client,
    base_url: String,
}

impl OrchestratorClient {
    /// Create a new orchestrator client with the specified base URL.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use orchestrator::client::OrchestratorClient;
    ///
    /// let client = OrchestratorClient::new("http://localhost:7006");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    // -- Agent operations --

    /// Check the health of the orchestrator service.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
    }

    /// List all agents, optionally filtered by status.
    pub async fn list_agents(
        &self,
        status: Option<&str>,
    ) -> Result<PaginatedResponse<AgentResponse>> {
        let path = match status {
            Some(s) => format!("/agents?status={}", s),
            None => "/agents".to_string(),
        };
        self.get(&path).await
    }

    /// Create a new agent.
    pub async fn create_agent(&self, request: &CreateAgentRequest) -> Result<AgentResponse> {
        self.post("/agents", request).await
    }

    /// Get a specific agent by ID.
    pub async fn get_agent(&self, id: &Uuid) -> Result<AgentResponse> {
        self.get(&format!("/agents/{}", id)).await
    }

    /// Terminate and remove an agent by ID.
    pub async fn terminate_agent(&self, id: &Uuid) -> Result<AgentResponse> {
        self.delete_with_response(&format!("/agents/{}", id)).await
    }

    /// Send a message (prompt) to a running non-interactive agent.
    pub async fn send_message(
        &self,
        id: &Uuid,
        request: &SendMessageRequest,
    ) -> Result<SendMessageResponse> {
        self.post(&format!("/agents/{}/message", id), request).await
    }

    /// Get the tool policy for an agent.
    pub async fn get_agent_policy(&self, id: &Uuid) -> Result<ToolPolicy> {
        self.get(&format!("/agents/{}/policy", id)).await
    }

    /// Update the tool policy for an agent.
    pub async fn update_agent_policy(&self, id: &Uuid, policy: &ToolPolicy) -> Result<ToolPolicy> {
        self.put(&format!("/agents/{}/policy", id), policy).await
    }

    // -- Approval operations --

    /// List all pending tool approval requests.
    pub async fn list_approvals(
        &self,
        status: Option<&str>,
    ) -> Result<PaginatedResponse<PendingApproval>> {
        let path = match status {
            Some(s) => format!("/approvals?status={}", s),
            None => "/approvals?status=pending".to_string(),
        };
        self.get(&path).await
    }

    /// List approval requests for a specific agent.
    pub async fn list_agent_approvals(
        &self,
        agent_id: &Uuid,
        status: Option<&str>,
    ) -> Result<PaginatedResponse<PendingApproval>> {
        let path = match status {
            Some(s) => format!("/agents/{}/approvals?status={}", agent_id, s),
            None => format!("/agents/{}/approvals?status=pending", agent_id),
        };
        self.get(&path).await
    }

    /// Get a specific approval request.
    pub async fn get_approval(&self, id: &Uuid) -> Result<PendingApproval> {
        self.get(&format!("/approvals/{}", id)).await
    }

    /// Approve a pending tool request.
    pub async fn approve_tool(&self, id: &Uuid) -> Result<PendingApproval> {
        self.post(
            &format!("/approvals/{}/approve", id),
            &ApprovalActionRequest::default(),
        )
        .await
    }

    /// Deny a pending tool request.
    pub async fn deny_tool(&self, id: &Uuid) -> Result<PendingApproval> {
        self.post(
            &format!("/approvals/{}/deny", id),
            &ApprovalActionRequest::default(),
        )
        .await
    }

    // -- Workflow operations --

    /// List all workflows.
    pub async fn list_workflows(&self) -> Result<PaginatedResponse<WorkflowResponse>> {
        self.get("/workflows").await
    }

    /// Create a new workflow.
    pub async fn create_workflow(
        &self,
        request: &CreateWorkflowRequest,
    ) -> Result<WorkflowResponse> {
        self.post("/workflows", request).await
    }

    /// Get a specific workflow by ID.
    pub async fn get_workflow(&self, id: &Uuid) -> Result<WorkflowResponse> {
        self.get(&format!("/workflows/{}", id)).await
    }

    /// Update an existing workflow.
    pub async fn update_workflow(
        &self,
        id: &Uuid,
        request: &UpdateWorkflowRequest,
    ) -> Result<WorkflowResponse> {
        self.put(&format!("/workflows/{}", id), request).await
    }

    /// Delete a workflow by ID.
    pub async fn delete_workflow(&self, id: &Uuid) -> Result<()> {
        self.delete(&format!("/workflows/{}", id)).await
    }

    /// Get the dispatch history for a workflow.
    pub async fn dispatch_history(&self, id: &Uuid) -> Result<PaginatedResponse<DispatchResponse>> {
        self.get(&format!("/workflows/{}/history", id)).await
    }

    // -- Private HTTP helpers --

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;
        Self::handle_response(response).await
    }

    async fn post<T: Serialize, R: DeserializeOwned>(&self, path: &str, body: &T) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;
        Self::handle_response(response).await
    }

    async fn put<T: Serialize, R: DeserializeOwned>(&self, path: &str, body: &T) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to PUT {url}"))?;
        Self::handle_response(response).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.delete(&url).send().await.context(format!("Failed to DELETE {url}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {error_text}"))
        }
    }

    async fn delete_with_response<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.delete(&url).send().await.context(format!("Failed to DELETE {url}"))?;
        Self::handle_response(response).await
    }

    async fn handle_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let status = response.status();
        if status.is_success() {
            response.json::<T>().await.context("Failed to parse response body")
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {error_text}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OrchestratorClient::new("http://localhost:7006");
        assert_eq!(client.base_url, "http://localhost:7006");
    }

    #[test]
    fn test_client_creation_with_string() {
        let client = OrchestratorClient::new("http://localhost:7006".to_string());
        assert_eq!(client.base_url, "http://localhost:7006");
    }

    #[test]
    fn test_client_clone() {
        let client1 = OrchestratorClient::new("http://localhost:7006");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
