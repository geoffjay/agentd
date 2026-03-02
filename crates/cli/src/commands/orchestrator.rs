//! Orchestrator service command implementations.
//!
//! This module implements all subcommands for managing agents and workflows via
//! the orchestrator service. The orchestrator manages AI agent lifecycles and
//! autonomous workflow scheduling.
//!
//! # Available Commands
//!
//! ## Agent Commands
//!
//! - **list-agents**: List all managed agents, optionally filtered by status
//! - **create-agent**: Create and spawn a new agent
//! - **get-agent**: Get details of a specific agent
//! - **delete-agent**: Terminate and remove an agent
//!
//! ## Workflow Commands
//!
//! - **list-workflows**: List all configured workflows
//! - **create-workflow**: Create a new autonomous workflow
//! - **get-workflow**: Get details of a specific workflow
//! - **update-workflow**: Update workflow configuration
//! - **delete-workflow**: Delete a workflow
//! - **workflow-history**: View dispatch history for a workflow
//!
//! # Examples
//!
//! ## List running agents
//!
//! ```bash
//! agent orchestrator list-agents --status running
//! ```
//!
//! ## Create an agent
//!
//! ```bash
//! agent orchestrator create-agent \
//!   --name my-agent \
//!   --working-dir /path/to/project \
//!   --prompt "Fix the bug in main.rs"
//! ```
//!
//! ## List workflows
//!
//! ```bash
//! agent orchestrator list-workflows
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;

use cli::client::ApiClient;

/// Orchestrator service management subcommands.
///
/// Each variant corresponds to a specific operation on the orchestrator service.
/// All commands communicate with the orchestrator service REST API.
#[derive(Subcommand)]
pub enum OrchestratorCommand {
    /// List all managed agents.
    ///
    /// Returns all agents tracked by the orchestrator, optionally filtered
    /// by status (pending, running, stopped, failed).
    ListAgents {
        /// Filter by agent status (pending, running, stopped, failed)
        #[arg(long)]
        status: Option<String>,
    },

    /// Create a new agent.
    ///
    /// Spawns a new AI agent in a tmux session managed by the orchestrator.
    CreateAgent {
        /// Agent name
        #[arg(long)]
        name: String,

        /// Working directory for the agent
        #[arg(long)]
        working_dir: String,

        /// OS user to run the agent as
        #[arg(long)]
        user: Option<String>,

        /// Shell to use (default: zsh)
        #[arg(long, default_value = "zsh")]
        shell: String,

        /// Start in interactive mode (no WebSocket)
        #[arg(long)]
        interactive: bool,

        /// Initial prompt for the agent
        #[arg(long)]
        prompt: Option<String>,

        /// Start the session with --worktree
        #[arg(long)]
        worktree: bool,

        /// System prompt for the session
        #[arg(long)]
        system_prompt: Option<String>,
    },

    /// Get details of a specific agent.
    GetAgent {
        /// Agent ID (UUID)
        id: String,
    },

    /// Terminate and remove an agent.
    DeleteAgent {
        /// Agent ID (UUID)
        id: String,
    },

    /// List all workflows.
    ListWorkflows,

    /// Create a new workflow.
    ///
    /// Creates an autonomous workflow that polls a task source and dispatches
    /// work to an agent.
    CreateWorkflow {
        /// Workflow name
        #[arg(long)]
        name: String,

        /// Agent ID to execute tasks
        #[arg(long)]
        agent_id: String,

        /// GitHub repository owner
        #[arg(long)]
        owner: String,

        /// GitHub repository name
        #[arg(long)]
        repo: String,

        /// Comma-separated labels to filter issues
        #[arg(long)]
        labels: Option<String>,

        /// Prompt template with {{placeholders}}
        #[arg(long)]
        prompt_template: String,

        /// Poll interval in seconds (default: 60)
        #[arg(long, default_value = "60")]
        poll_interval: u64,

        /// Whether the workflow is enabled (default: true)
        #[arg(long, default_value = "true")]
        enabled: bool,
    },

    /// Get details of a specific workflow.
    GetWorkflow {
        /// Workflow ID (UUID)
        id: String,
    },

    /// Update an existing workflow.
    UpdateWorkflow {
        /// Workflow ID (UUID)
        id: String,

        /// New workflow name
        #[arg(long)]
        name: Option<String>,

        /// New prompt template
        #[arg(long)]
        prompt_template: Option<String>,

        /// New poll interval in seconds
        #[arg(long)]
        poll_interval: Option<u64>,

        /// Enable or disable the workflow
        #[arg(long)]
        enabled: Option<bool>,
    },

    /// Delete a workflow.
    DeleteWorkflow {
        /// Workflow ID (UUID)
        id: String,
    },

    /// View dispatch history for a workflow.
    WorkflowHistory {
        /// Workflow ID (UUID)
        id: String,
    },
}

impl OrchestratorCommand {
    /// Execute the orchestrator command by dispatching to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `client` - The API client configured for the orchestrator service
    /// * `json` - If true, output raw JSON instead of formatted text
    pub async fn execute(&self, client: &ApiClient, json: bool) -> Result<()> {
        match self {
            OrchestratorCommand::ListAgents { status } => {
                list_agents(client, status.as_deref(), json).await
            }
            OrchestratorCommand::CreateAgent {
                name,
                working_dir,
                user,
                shell,
                interactive,
                prompt,
                worktree,
                system_prompt,
            } => {
                create_agent(
                    client,
                    name,
                    working_dir,
                    user.as_deref(),
                    shell,
                    *interactive,
                    prompt.as_deref(),
                    *worktree,
                    system_prompt.as_deref(),
                    json,
                )
                .await
            }
            OrchestratorCommand::GetAgent { id } => get_agent(client, id, json).await,
            OrchestratorCommand::DeleteAgent { id } => delete_agent(client, id, json).await,
            OrchestratorCommand::ListWorkflows => list_workflows(client, json).await,
            OrchestratorCommand::CreateWorkflow {
                name,
                agent_id,
                owner,
                repo,
                labels,
                prompt_template,
                poll_interval,
                enabled,
            } => {
                create_workflow(
                    client,
                    name,
                    agent_id,
                    owner,
                    repo,
                    labels.as_deref(),
                    prompt_template,
                    *poll_interval,
                    *enabled,
                    json,
                )
                .await
            }
            OrchestratorCommand::GetWorkflow { id } => get_workflow(client, id, json).await,
            OrchestratorCommand::UpdateWorkflow {
                id,
                name,
                prompt_template,
                poll_interval,
                enabled,
            } => {
                update_workflow(
                    client,
                    id,
                    name.as_deref(),
                    prompt_template.as_deref(),
                    *poll_interval,
                    *enabled,
                    json,
                )
                .await
            }
            OrchestratorCommand::DeleteWorkflow { id } => delete_workflow(client, id, json).await,
            OrchestratorCommand::WorkflowHistory { id } => workflow_history(client, id, json).await,
        }
    }
}

// -- Agent operations --

async fn list_agents(client: &ApiClient, status: Option<&str>, json: bool) -> Result<()> {
    let path = match status {
        Some(s) => format!("/agents?status={}", s),
        None => "/agents".to_string(),
    };

    let agents: Vec<serde_json::Value> =
        client.get(&path).await.context("Failed to list agents")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agents)?);
    } else if agents.is_empty() {
        println!("{}", "No agents found.".yellow());
    } else {
        println!("{}", "Agents:".blue().bold());
        println!("{}", "=".repeat(80).cyan());
        for agent in &agents {
            display_agent(agent);
            println!("{}", "-".repeat(80).cyan());
        }
        println!("Total: {} agent(s)", agents.len());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_agent(
    client: &ApiClient,
    name: &str,
    working_dir: &str,
    user: Option<&str>,
    shell: &str,
    interactive: bool,
    prompt: Option<&str>,
    worktree: bool,
    system_prompt: Option<&str>,
    json: bool,
) -> Result<()> {
    let body = serde_json::json!({
        "name": name,
        "working_dir": working_dir,
        "user": user,
        "shell": shell,
        "interactive": interactive,
        "prompt": prompt,
        "worktree": worktree,
        "system_prompt": system_prompt,
    });

    let agent: serde_json::Value =
        client.post("/agents", &body).await.context("Failed to create agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        println!("{}", "Agent created successfully!".green().bold());
        println!();
        display_agent(&agent);
    }

    Ok(())
}

async fn get_agent(client: &ApiClient, id: &str, json: bool) -> Result<()> {
    let path = format!("/agents/{}", id);
    let agent: serde_json::Value = client.get(&path).await.context("Failed to get agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        display_agent(&agent);
    }

    Ok(())
}

async fn delete_agent(client: &ApiClient, id: &str, json: bool) -> Result<()> {
    let path = format!("/agents/{}", id);
    let agent: serde_json::Value = client.get(&path).await.context("Failed to find agent")?;

    // The orchestrator DELETE returns the terminated agent
    let result: serde_json::Value =
        client.get::<serde_json::Value>(&path).await.ok().unwrap_or_default();

    client.delete(&path).await.context("Failed to delete agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        let name = result.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        println!("{}", format!("Agent '{}' ({}) terminated.", name, id).green().bold());
    }

    Ok(())
}

// -- Workflow operations --

async fn list_workflows(client: &ApiClient, json: bool) -> Result<()> {
    let workflows: Vec<serde_json::Value> =
        client.get("/workflows").await.context("Failed to list workflows")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflows)?);
    } else if workflows.is_empty() {
        println!("{}", "No workflows found.".yellow());
    } else {
        println!("{}", "Workflows:".blue().bold());
        println!("{}", "=".repeat(80).cyan());
        for workflow in &workflows {
            display_workflow(workflow);
            println!("{}", "-".repeat(80).cyan());
        }
        println!("Total: {} workflow(s)", workflows.len());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_workflow(
    client: &ApiClient,
    name: &str,
    agent_id: &str,
    owner: &str,
    repo: &str,
    labels: Option<&str>,
    prompt_template: &str,
    poll_interval: u64,
    enabled: bool,
    json: bool,
) -> Result<()> {
    let labels_vec: Vec<String> =
        labels.map(|l| l.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();

    let body = serde_json::json!({
        "name": name,
        "agent_id": agent_id,
        "source_config": {
            "type": "github_issues",
            "owner": owner,
            "repo": repo,
            "labels": labels_vec,
        },
        "prompt_template": prompt_template,
        "poll_interval_secs": poll_interval,
        "enabled": enabled,
    });

    let workflow: serde_json::Value =
        client.post("/workflows", &body).await.context("Failed to create workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!("{}", "Workflow created successfully!".green().bold());
        println!();
        display_workflow(&workflow);
    }

    Ok(())
}

async fn get_workflow(client: &ApiClient, id: &str, json: bool) -> Result<()> {
    let path = format!("/workflows/{}", id);
    let workflow: serde_json::Value = client.get(&path).await.context("Failed to get workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        display_workflow(&workflow);
    }

    Ok(())
}

async fn update_workflow(
    client: &ApiClient,
    id: &str,
    name: Option<&str>,
    prompt_template: Option<&str>,
    poll_interval: Option<u64>,
    enabled: Option<bool>,
    json: bool,
) -> Result<()> {
    let body = serde_json::json!({
        "name": name,
        "prompt_template": prompt_template,
        "poll_interval_secs": poll_interval,
        "enabled": enabled,
    });

    let path = format!("/workflows/{}", id);
    let workflow: serde_json::Value =
        client.put(&path, &body).await.context("Failed to update workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!("{}", "Workflow updated successfully!".green().bold());
        println!();
        display_workflow(&workflow);
    }

    Ok(())
}

async fn delete_workflow(client: &ApiClient, id: &str, json: bool) -> Result<()> {
    let path = format!("/workflows/{}", id);

    if json {
        // Fetch before deleting to show it
        if let Ok(workflow) = client.get::<serde_json::Value>(&path).await {
            client.delete(&path).await.context("Failed to delete workflow")?;
            println!("{}", serde_json::to_string_pretty(&workflow)?);
        } else {
            client.delete(&path).await.context("Failed to delete workflow")?;
            println!("{{}}");
        }
    } else {
        client.delete(&path).await.context("Failed to delete workflow")?;
        println!("{}", format!("Workflow {} deleted.", id).green().bold());
    }

    Ok(())
}

async fn workflow_history(client: &ApiClient, id: &str, json: bool) -> Result<()> {
    let path = format!("/workflows/{}/history", id);
    let dispatches: Vec<serde_json::Value> =
        client.get(&path).await.context("Failed to get workflow history")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&dispatches)?);
    } else if dispatches.is_empty() {
        println!("{}", "No dispatch history found.".yellow());
    } else {
        println!("{}", "Dispatch History:".blue().bold());
        println!("{}", "=".repeat(80).cyan());
        for dispatch in &dispatches {
            display_dispatch(dispatch);
            println!("{}", "-".repeat(80).cyan());
        }
        println!("Total: {} dispatch(es)", dispatches.len());
    }

    Ok(())
}

// -- Display helpers --

fn display_agent(agent: &serde_json::Value) {
    if let Some(id) = agent.get("id").and_then(|v| v.as_str()) {
        println!("{}: {}", "ID".bold(), id);
    }
    if let Some(name) = agent.get("name").and_then(|v| v.as_str()) {
        println!("{}: {}", "Name".bold(), name.bright_white());
    }
    if let Some(status) = agent.get("status").and_then(|v| v.as_str()) {
        let colored_status = match status {
            "running" => status.green(),
            "pending" => status.yellow(),
            "stopped" => status.red(),
            "failed" => status.bright_red(),
            _ => status.normal(),
        };
        println!("{}: {}", "Status".bold(), colored_status);
    }
    if let Some(session) = agent.get("tmux_session").and_then(|v| v.as_str()) {
        println!("{}: {}", "Tmux Session".bold(), session);
    }
    if let Some(config) = agent.get("config") {
        if let Some(dir) = config.get("working_dir").and_then(|v| v.as_str()) {
            println!("{}: {}", "Working Dir".bold(), dir);
        }
        if let Some(shell) = config.get("shell").and_then(|v| v.as_str()) {
            println!("{}: {}", "Shell".bold(), shell);
        }
    }
    if let Some(created) = agent.get("created_at").and_then(|v| v.as_str()) {
        println!("{}: {}", "Created".bold(), created);
    }
}

fn display_workflow(workflow: &serde_json::Value) {
    if let Some(id) = workflow.get("id").and_then(|v| v.as_str()) {
        println!("{}: {}", "ID".bold(), id);
    }
    if let Some(name) = workflow.get("name").and_then(|v| v.as_str()) {
        println!("{}: {}", "Name".bold(), name.bright_white());
    }
    if let Some(agent_id) = workflow.get("agent_id").and_then(|v| v.as_str()) {
        println!("{}: {}", "Agent ID".bold(), agent_id);
    }
    if let Some(enabled) = workflow.get("enabled").and_then(|v| v.as_bool()) {
        let status = if enabled { "enabled".green() } else { "disabled".red() };
        println!("{}: {}", "Status".bold(), status);
    }
    if let Some(interval) = workflow.get("poll_interval_secs").and_then(|v| v.as_u64()) {
        println!("{}: {}s", "Poll Interval".bold(), interval);
    }
    if let Some(source) = workflow.get("source_config") {
        if let Some(stype) = source.get("type").and_then(|v| v.as_str()) {
            println!("{}: {}", "Source Type".bold(), stype);
        }
        if let (Some(owner), Some(repo)) = (
            source.get("owner").and_then(|v| v.as_str()),
            source.get("repo").and_then(|v| v.as_str()),
        ) {
            println!("{}: {}/{}", "Repository".bold(), owner, repo);
        }
    }
    if let Some(template) = workflow.get("prompt_template").and_then(|v| v.as_str()) {
        let display = if template.len() > 60 {
            format!("{}...", &template[..57])
        } else {
            template.to_string()
        };
        println!("{}: {}", "Prompt Template".bold(), display);
    }
    if let Some(created) = workflow.get("created_at").and_then(|v| v.as_str()) {
        println!("{}: {}", "Created".bold(), created);
    }
}

fn display_dispatch(dispatch: &serde_json::Value) {
    if let Some(id) = dispatch.get("id").and_then(|v| v.as_str()) {
        println!("{}: {}", "Dispatch ID".bold(), id);
    }
    if let Some(source_id) = dispatch.get("source_id").and_then(|v| v.as_str()) {
        println!("{}: {}", "Source ID".bold(), source_id);
    }
    if let Some(status) = dispatch.get("status").and_then(|v| v.as_str()) {
        let colored_status = match status {
            "completed" => status.green(),
            "dispatched" => status.blue(),
            "pending" => status.yellow(),
            "failed" => status.bright_red(),
            "skipped" => status.normal(),
            _ => status.normal(),
        };
        println!("{}: {}", "Status".bold(), colored_status);
    }
    if let Some(prompt) = dispatch.get("prompt_sent").and_then(|v| v.as_str()) {
        let display =
            if prompt.len() > 60 { format!("{}...", &prompt[..57]) } else { prompt.to_string() };
        println!("{}: {}", "Prompt".bold(), display);
    }
    if let Some(dispatched) = dispatch.get("dispatched_at").and_then(|v| v.as_str()) {
        println!("{}: {}", "Dispatched".bold(), dispatched);
    }
    if let Some(completed) = dispatch.get("completed_at").and_then(|v| v.as_str()) {
        println!("{}: {}", "Completed".bold(), completed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_agent_minimal() {
        let agent = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test-agent",
            "status": "running",
        });
        // Should not panic
        display_agent(&agent);
    }

    #[test]
    fn test_display_workflow_minimal() {
        let workflow = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440001",
            "name": "test-workflow",
            "enabled": true,
        });
        // Should not panic
        display_workflow(&workflow);
    }

    #[test]
    fn test_display_dispatch_minimal() {
        let dispatch = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440002",
            "source_id": "issue-42",
            "status": "completed",
        });
        // Should not panic
        display_dispatch(&dispatch);
    }

    #[test]
    fn test_display_agent_with_config() {
        let agent = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "my-agent",
            "status": "pending",
            "config": {
                "working_dir": "/tmp/test",
                "shell": "zsh",
            },
            "tmux_session": "agentd-orch-abc123",
            "created_at": "2024-01-01T00:00:00Z",
        });
        // Should not panic
        display_agent(&agent);
    }

    #[test]
    fn test_display_workflow_full() {
        let workflow = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440001",
            "name": "issue-worker",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "enabled": false,
            "poll_interval_secs": 120,
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets",
            },
            "prompt_template": "Fix the issue: {{title}}",
            "created_at": "2024-01-01T00:00:00Z",
        });
        // Should not panic
        display_workflow(&workflow);
    }

    #[test]
    fn test_display_workflow_long_template() {
        let workflow = serde_json::json!({
            "prompt_template": "This is a very long prompt template that should be truncated when displayed in the terminal output",
        });
        // Should not panic and should truncate
        display_workflow(&workflow);
    }
}
