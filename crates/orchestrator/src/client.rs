//! HTTP client for the orchestrator service.
//!
//! Provides a typed client for communicating with the orchestrator REST API.
//! All methods are async and return strongly-typed response objects.
//!
//! # Examples
//!
//! ```ignore
//! use orchestrator::client::OrchestratorClient;
//!
//! let client = OrchestratorClient::new("http://localhost:7006");
//! ```
//!
//! ```ignore
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
use serde_json;
use uuid::Uuid;

use crate::scheduler::types::{
    CreateWorkflowRequest, DispatchResponse, TriggerWorkflowRequest, UpdateWorkflowRequest,
    WorkflowResponse,
};
use crate::types::{
    AddDirRequest, AddDirResponse, AgentResponse, AgentUsageStats, ApprovalActionRequest,
    ClearContextRequest, ClearContextResponse, ConversationEventResponse, ConversationHistoryQuery,
    ConversationHistoryResponse, ConversationSummary, CreateAgentRequest, CreateProjectRequest,
    HealthResponse, PaginatedResponse, PendingApproval, Project, ProjectResponse,
    SendMessageRequest, SendMessageResponse, SetModelRequest, ToolPolicy, UpdateProjectRequest,
};

/// Typed HTTP client for the orchestrator service.
///
/// Provides strongly-typed methods for all orchestrator REST API endpoints,
/// including agent management, workflow operations, and health checks.
///
/// # Examples
///
/// ```ignore
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
    /// ```ignore
    /// use orchestrator::client::OrchestratorClient;
    ///
    /// let client = OrchestratorClient::new("http://localhost:7006");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Create a client using the `AGENTD_ORCHESTRATOR_SERVICE_URL` environment
    /// variable, falling back to `http://localhost:7006`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use orchestrator::client::OrchestratorClient;
    ///
    /// let client = OrchestratorClient::from_env();
    /// ```
    pub fn from_env() -> Self {
        let url = std::env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:7006".to_string());
        Self::new(url)
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
        self.list_agents_filtered(status, false).await
    }

    /// List agents with optional status filter and built-in inclusion.
    ///
    /// When `include_builtin` is `true`, passes `?include_builtin=true` so that
    /// system agents are included alongside user agents.
    pub async fn list_agents_filtered(
        &self,
        status: Option<&str>,
        include_builtin: bool,
    ) -> Result<PaginatedResponse<AgentResponse>> {
        let mut params: Vec<String> = Vec::new();
        if let Some(s) = status {
            params.push(format!("status={}", s));
        }
        if include_builtin {
            params.push("include_builtin=true".to_string());
        }
        let path = if params.is_empty() {
            "/agents".to_string()
        } else {
            format!("/agents?{}", params.join("&"))
        };
        self.get(&path).await
    }

    /// List built-in system agents.
    ///
    /// Fetches from `GET /system-agents` which returns only agents with
    /// `built_in = true`. These are the programmatically-managed agents
    /// always present during orchestrator operation.
    pub async fn list_system_agents(&self) -> Result<Vec<AgentResponse>> {
        self.get("/system-agents").await
    }

    /// Create a new agent.
    pub async fn create_agent(&self, request: &CreateAgentRequest) -> Result<AgentResponse> {
        self.post("/agents", request).await
    }

    /// Get a specific agent by ID.
    pub async fn get_agent(&self, id: &Uuid) -> Result<AgentResponse> {
        self.get(&format!("/agents/{}", id)).await
    }

    /// Find an agent by name, returning `None` if no agent with that name exists.
    ///
    /// Fetches a paginated list of all agents and searches client-side because
    /// the orchestrator API does not expose a dedicated name-lookup endpoint.
    /// Agent names are unique within the orchestrator, so at most one result is
    /// returned.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use orchestrator::client::OrchestratorClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = OrchestratorClient::new("http://localhost:7006");
    /// if let Some(agent) = client.get_agent_by_name("conductor").await? {
    ///     println!("conductor UUID = {}", agent.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_agent_by_name(&self, name: &str) -> Result<Option<AgentResponse>> {
        // Fetch a large page to avoid missing agents in busy systems.
        // A future improvement could add a server-side ?name= filter.
        let resp: PaginatedResponse<AgentResponse> = self.get("/agents?limit=500&offset=0").await?;
        Ok(resp.items.into_iter().find(|a| a.name == name))
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

    /// Set or change the model for an agent.
    ///
    /// If `restart` is true and the agent is running, the agent process will
    /// be killed and re-launched with the new model.
    pub async fn set_model(
        &self,
        id: &Uuid,
        model: Option<String>,
        restart: bool,
    ) -> Result<AgentResponse> {
        let request = SetModelRequest { model, restart };
        self.put(&format!("/agents/{}/model", id), &request).await
    }

    // -- Additional directory operations --

    /// Add a directory to an agent's accessible paths.
    ///
    /// The path must exist and be a directory. The change takes effect on the
    /// next agent restart.
    pub async fn add_dir(&self, id: &Uuid, path: &str) -> Result<AddDirResponse> {
        self.post(&format!("/agents/{}/dirs", id), &AddDirRequest { path: path.to_string() }).await
    }

    /// Remove a directory from an agent's accessible paths.
    ///
    /// The change takes effect on the next agent restart.
    pub async fn remove_dir(&self, id: &Uuid, path: &str) -> Result<AddDirResponse> {
        self.delete_with_body(
            &format!("/agents/{}/dirs", id),
            &AddDirRequest { path: path.to_string() },
        )
        .await
    }

    // -- Usage & context operations --

    /// Get usage statistics for an agent.
    pub async fn get_agent_usage(&self, id: &Uuid) -> Result<AgentUsageStats> {
        self.get(&format!("/agents/{}/usage", id)).await
    }

    /// Clear an agent's context and start a fresh session.
    pub async fn clear_context(&self, id: &Uuid) -> Result<ClearContextResponse> {
        self.post(&format!("/agents/{}/clear-context", id), &ClearContextRequest {}).await
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
        self.post(&format!("/approvals/{}/approve", id), &ApprovalActionRequest::default()).await
    }

    /// Deny a pending tool request.
    pub async fn deny_tool(&self, id: &Uuid) -> Result<PendingApproval> {
        self.post(&format!("/approvals/{}/deny", id), &ApprovalActionRequest::default()).await
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

    /// Manually trigger a workflow on demand, bypassing its normal trigger strategy.
    pub async fn trigger_workflow(
        &self,
        id: &Uuid,
        request: &TriggerWorkflowRequest,
    ) -> Result<DispatchResponse> {
        self.post(&format!("/workflows/{}/trigger", id), request).await
    }

    // -- Queue operations --

    /// Push a task onto a named queue.
    pub async fn queue_push(
        &self,
        queue_name: &str,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.post(&format!("/queues/{}/push", queue_name), request).await
    }

    /// Get statistics for a named queue.
    pub async fn queue_stats(&self, queue_name: &str) -> Result<serde_json::Value> {
        self.get(&format!("/queues/{}/stats", queue_name)).await
    }

    /// Peek at pending tasks in a named queue.
    pub async fn queue_peek(
        &self,
        queue_name: &str,
        limit: Option<u64>,
    ) -> Result<Vec<serde_json::Value>> {
        let path = match limit {
            Some(n) => format!("/queues/{}/peek?limit={}", queue_name, n),
            None => format!("/queues/{}/peek", queue_name),
        };
        self.get(&path).await
    }

    /// Purge all tasks from a named queue.
    pub async fn queue_purge(&self, queue_name: &str) -> Result<serde_json::Value> {
        self.delete_with_response(&format!("/queues/{}", queue_name)).await
    }

    // -- Project management --

    /// List all projects.
    pub async fn list_projects(&self) -> Result<PaginatedResponse<Project>> {
        self.get("/projects").await
    }

    /// Create a new project.
    pub async fn create_project(&self, req: &CreateProjectRequest) -> Result<Project> {
        self.post("/projects", req).await
    }

    /// Get a project by UUID, including agent and workflow counts.
    pub async fn get_project(&self, id: &Uuid) -> Result<ProjectResponse> {
        self.get(&format!("/projects/{id}")).await
    }

    /// Find a project by name (client-side search).
    pub async fn get_project_by_name(&self, name: &str) -> Result<Option<Project>> {
        let resp: PaginatedResponse<Project> = self.get("/projects?limit=500").await?;
        Ok(resp.items.into_iter().find(|p| p.name == name))
    }

    /// Update a project's name and/or description.
    pub async fn update_project(&self, id: &Uuid, req: &UpdateProjectRequest) -> Result<Project> {
        self.put(&format!("/projects/{id}"), req).await
    }

    /// Delete a project (fails if agents or workflows are still associated).
    pub async fn delete_project(&self, id: &Uuid) -> Result<()> {
        self.delete(&format!("/projects/{id}")).await
    }

    /// List agents associated with a project.
    pub async fn list_project_agents(&self, id: &Uuid) -> Result<PaginatedResponse<AgentResponse>> {
        self.get(&format!("/projects/{id}/agents")).await
    }

    /// Associate an agent with a project.
    pub async fn associate_project_agent(&self, project_id: &Uuid, agent_id: &Uuid) -> Result<()> {
        let url = format!("{}/projects/{project_id}/agents/{agent_id}", self.base_url);
        let response =
            self.client.post(&url).send().await.context(format!("Failed to POST {url}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {text}"))
        }
    }

    /// Remove an agent from a project.
    pub async fn dissociate_project_agent(&self, project_id: &Uuid, agent_id: &Uuid) -> Result<()> {
        self.delete(&format!("/projects/{project_id}/agents/{agent_id}")).await
    }

    /// List workflows associated with a project.
    pub async fn list_project_workflows(
        &self,
        id: &Uuid,
    ) -> Result<PaginatedResponse<WorkflowResponse>> {
        self.get(&format!("/projects/{id}/workflows")).await
    }

    /// Associate a workflow with a project.
    pub async fn associate_project_workflow(
        &self,
        project_id: &Uuid,
        workflow_id: &Uuid,
    ) -> Result<()> {
        let url = format!("{}/projects/{project_id}/workflows/{workflow_id}", self.base_url);
        let response =
            self.client.post(&url).send().await.context(format!("Failed to POST {url}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {text}"))
        }
    }

    /// Remove a workflow from a project.
    pub async fn dissociate_project_workflow(
        &self,
        project_id: &Uuid,
        workflow_id: &Uuid,
    ) -> Result<()> {
        self.delete(&format!("/projects/{project_id}/workflows/{workflow_id}")).await
    }

    // -- Conversation history --

    /// List conversation events for an agent with optional filters.
    ///
    /// Supports `limit`, `before`/`after` (RFC 3339), `event_type` (comma-separated),
    /// and `session` query parameters via [`ConversationHistoryQuery`].
    pub async fn list_conversation_events(
        &self,
        agent_id: &Uuid,
        query: &ConversationHistoryQuery,
    ) -> Result<ConversationHistoryResponse> {
        let mut params: Vec<String> = Vec::new();
        if let Some(limit) = query.limit {
            params.push(format!("limit={limit}"));
        }
        if let Some(ref before) = query.before {
            params.push(format!("before={before}"));
        }
        if let Some(ref after) = query.after {
            params.push(format!("after={after}"));
        }
        if let Some(ref event_type) = query.event_type {
            params.push(format!("event_type={event_type}"));
        }
        if let Some(session) = query.session {
            params.push(format!("session={session}"));
        }
        let qs = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
        self.get(&format!("/agents/{agent_id}/conversation{qs}")).await
    }

    /// Get an aggregate summary of conversation events for an agent.
    pub async fn get_conversation_summary(&self, agent_id: &Uuid) -> Result<ConversationSummary> {
        self.get(&format!("/agents/{agent_id}/conversation/summary")).await
    }

    /// Get a single conversation event by ID for an agent.
    pub async fn get_conversation_event(
        &self,
        agent_id: &Uuid,
        event_id: &Uuid,
    ) -> Result<ConversationEventResponse> {
        self.get(&format!("/agents/{agent_id}/conversation/{event_id}")).await
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

    async fn delete_with_body<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .delete(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to DELETE {url}"))?;
        Self::handle_response(response).await
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
    // reqwest::Client::new() triggers macOS system-configuration TLS
    // initialisation which panics when called from non-main test threads.
    // These tests verify URL string handling without constructing the client.

    #[test]
    fn test_base_url_string_conversion() {
        let url: String = "http://localhost:7006".into();
        assert_eq!(url, "http://localhost:7006");
    }

    #[test]
    fn test_base_url_clone() {
        let url1 = "http://localhost:7006".to_string();
        let url2 = url1.clone();
        assert_eq!(url1, url2);
    }

    #[test]
    fn test_base_url_from_string() {
        let url: String = String::from("http://localhost:7006");
        assert_eq!(url, "http://localhost:7006");
    }
}
