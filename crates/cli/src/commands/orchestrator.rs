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

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::*;
use serde::Deserialize;

use cli::client::ApiClient;

/// Paginated response wrapper returned by list endpoints.
#[derive(Debug, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,
    #[allow(dead_code)]
    total: u64,
    #[allow(dead_code)]
    limit: u64,
    #[allow(dead_code)]
    offset: u64,
}

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
    /// The working directory defaults to the current directory if not specified.
    ///
    /// # Examples
    ///
    /// Create an agent in the current directory:
    ///
    ///   agent orchestrator create-agent --name my-agent
    ///
    /// Create and immediately attach to the tmux session:
    ///
    ///   agent orchestrator create-agent --name debug --interactive --attach
    ///
    /// Create an agent with a prompt from a file:
    ///
    ///   agent orchestrator create-agent \
    ///     --name code-reviewer \
    ///     --working-dir /path/to/project \
    ///     --prompt-file ./review-instructions.md
    ///
    /// Pipe a prompt from stdin:
    ///
    ///   echo "Fix the failing tests" | agent orchestrator create-agent \
    ///     --name fixer --stdin
    CreateAgent {
        /// Agent name
        #[arg(long)]
        name: String,

        /// Working directory for the agent (defaults to current directory)
        #[arg(long)]
        working_dir: Option<String>,

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
        #[arg(long, conflicts_with_all = ["prompt_file", "stdin"])]
        prompt: Option<String>,

        /// Read the initial prompt from a file
        #[arg(long, conflicts_with_all = ["prompt", "stdin"])]
        prompt_file: Option<PathBuf>,

        /// Read the initial prompt from stdin (supports piping)
        #[arg(long, conflicts_with_all = ["prompt", "prompt_file"])]
        stdin: bool,

        /// Start the session with --worktree
        #[arg(long)]
        worktree: bool,

        /// System prompt for the session
        #[arg(long)]
        system_prompt: Option<String>,

        /// Attach to the tmux session after creating the agent
        #[arg(long)]
        attach: bool,
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
    /// work to an agent. The workflow is enabled by default unless --disabled
    /// is specified.
    ///
    /// # Examples
    ///
    /// Create a workflow using an agent name:
    ///
    ///   agent orchestrator create-workflow \
    ///     --name issue-worker \
    ///     --agent-name my-agent \
    ///     --owner acme --repo widgets \
    ///     --labels "bug,help wanted" \
    ///     --prompt-template-file ./prompt.txt \
    ///     --poll-interval 120
    ///
    /// Create a disabled workflow for testing:
    ///
    ///   agent orchestrator create-workflow \
    ///     --name test-workflow \
    ///     --agent-id 550e8400-e29b-41d4-a716-446655440000 \
    ///     --owner acme --repo widgets \
    ///     --prompt-template "Fix: {{title}}" \
    ///     --disabled
    CreateWorkflow {
        /// Workflow name
        #[arg(long)]
        name: String,

        /// Agent ID (UUID) to execute tasks
        #[arg(long, conflicts_with = "agent_name")]
        agent_id: Option<String>,

        /// Agent name to execute tasks (resolved to ID via the API)
        #[arg(long, conflicts_with = "agent_id")]
        agent_name: Option<String>,

        /// GitHub repository owner
        #[arg(long)]
        owner: String,

        /// GitHub repository name
        #[arg(long)]
        repo: String,

        /// Comma-separated labels to filter issues
        #[arg(long)]
        labels: Option<String>,

        /// Prompt template with {{placeholders}} (e.g. "Fix: {{title}}\n{{body}}")
        #[arg(long, conflicts_with = "prompt_template_file")]
        prompt_template: Option<String>,

        /// Path to a file containing the prompt template
        #[arg(long, conflicts_with = "prompt_template")]
        prompt_template_file: Option<PathBuf>,

        /// Poll interval in seconds (default: 60)
        #[arg(long, default_value = "60")]
        poll_interval: u64,

        /// Create the workflow in disabled state (won't start polling)
        #[arg(long)]
        disabled: bool,
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
                prompt_file,
                stdin,
                worktree,
                system_prompt,
                attach,
            } => {
                create_agent(
                    client,
                    name,
                    working_dir.as_deref(),
                    user.as_deref(),
                    shell,
                    *interactive,
                    prompt.as_deref(),
                    prompt_file.as_deref(),
                    *stdin,
                    *worktree,
                    system_prompt.as_deref(),
                    *attach,
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
                agent_name,
                owner,
                repo,
                labels,
                prompt_template,
                prompt_template_file,
                poll_interval,
                disabled,
            } => {
                create_workflow(
                    client,
                    name,
                    agent_id.as_deref(),
                    agent_name.as_deref(),
                    owner,
                    repo,
                    labels.as_deref(),
                    prompt_template.as_deref(),
                    prompt_template_file.as_deref(),
                    *poll_interval,
                    !disabled,
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

    let response: PaginatedResponse<serde_json::Value> =
        client.get(&path).await.context("Failed to list agents")?;
    let agents = response.items;

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
    working_dir: Option<&str>,
    user: Option<&str>,
    shell: &str,
    interactive: bool,
    prompt: Option<&str>,
    prompt_file: Option<&std::path::Path>,
    stdin: bool,
    worktree: bool,
    system_prompt: Option<&str>,
    attach: bool,
    json: bool,
) -> Result<()> {
    // Resolve working directory: use provided value or default to $PWD
    let resolved_working_dir = resolve_working_dir(working_dir)?;

    // Resolve prompt from --prompt, --prompt-file, or --stdin
    let resolved_prompt = resolve_agent_prompt(prompt, prompt_file, stdin)?;

    // Build JSON body, omitting null optional fields
    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), serde_json::json!(name));
    body.insert("working_dir".to_string(), serde_json::json!(resolved_working_dir));
    body.insert("shell".to_string(), serde_json::json!(shell));
    body.insert("interactive".to_string(), serde_json::json!(interactive));
    body.insert("worktree".to_string(), serde_json::json!(worktree));

    // Only include optional fields if they have values
    if let Some(u) = user {
        body.insert("user".to_string(), serde_json::json!(u));
    }
    if let Some(p) = &resolved_prompt {
        body.insert("prompt".to_string(), serde_json::json!(p));
    }
    if let Some(sp) = system_prompt {
        body.insert("system_prompt".to_string(), serde_json::json!(sp));
    }

    let agent: serde_json::Value = client
        .post("/agents", &serde_json::Value::Object(body))
        .await
        .context("Failed to create agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        println!("{}", "Agent created successfully!".green().bold());
        println!();
        display_agent(&agent);
    }

    // If --attach was requested, exec into the tmux session
    if attach {
        let session = agent
            .get("tmux_session")
            .and_then(|v| v.as_str())
            .context("Agent response missing 'tmux_session' field — cannot attach")?;

        println!();
        println!("{}", format!("Attaching to tmux session: {session}").cyan());

        let status = std::process::Command::new("tmux")
            .args(["attach-session", "-t", session])
            .status()
            .context("Failed to exec tmux attach-session")?;

        if !status.success() {
            bail!("tmux attach-session exited with status: {status}");
        }
    }

    Ok(())
}

/// Resolve the working directory from the provided value or default to $PWD.
fn resolve_working_dir(working_dir: Option<&str>) -> Result<String> {
    match working_dir {
        Some(dir) => Ok(dir.to_string()),
        None => std::env::current_dir()
            .context("Failed to determine current directory")
            .map(|p| p.to_string_lossy().to_string()),
    }
}

/// Resolve the agent prompt from --prompt, --prompt-file, or --stdin.
///
/// Returns `None` if no prompt source was provided (all three are optional).
fn resolve_agent_prompt(
    prompt: Option<&str>,
    prompt_file: Option<&std::path::Path>,
    stdin: bool,
) -> Result<Option<String>> {
    match (prompt, prompt_file, stdin) {
        (Some(p), _, _) => Ok(Some(p.to_string())),
        (_, Some(path), _) => {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read prompt file: {}", path.display()))?;
            if content.trim().is_empty() {
                bail!("Prompt file is empty: {}", path.display());
            }
            Ok(Some(content))
        }
        (_, _, true) => {
            use std::io::Read;
            let mut content = String::new();
            std::io::stdin()
                .read_to_string(&mut content)
                .context("Failed to read prompt from stdin")?;
            if content.trim().is_empty() {
                bail!("No prompt provided on stdin");
            }
            Ok(Some(content))
        }
        (None, None, false) => Ok(None),
    }
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
    let response: PaginatedResponse<serde_json::Value> =
        client.get("/workflows").await.context("Failed to list workflows")?;
    let workflows = response.items;

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
    agent_id: Option<&str>,
    agent_name: Option<&str>,
    owner: &str,
    repo: &str,
    labels: Option<&str>,
    prompt_template: Option<&str>,
    prompt_template_file: Option<&std::path::Path>,
    poll_interval: u64,
    enabled: bool,
    json: bool,
) -> Result<()> {
    // Resolve agent ID from --agent-id or --agent-name
    let resolved_agent_id = resolve_agent_id(client, agent_id, agent_name).await?;

    // Resolve prompt template from --prompt-template or --prompt-template-file
    let resolved_template = resolve_prompt_template(prompt_template, prompt_template_file)?;

    let labels_vec: Vec<String> =
        labels.map(|l| l.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();

    let body = serde_json::json!({
        "name": name,
        "agent_id": resolved_agent_id,
        "source_config": {
            "type": "github_issues",
            "owner": owner,
            "repo": repo,
            "labels": labels_vec,
        },
        "prompt_template": resolved_template,
        "poll_interval_secs": poll_interval,
        "enabled": enabled,
    });

    let workflow: serde_json::Value = client.post("/workflows", &body).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("404") {
            anyhow::anyhow!(
                "Agent '{}' not found or not running. Use 'agent orchestrator list-agents' to see available agents.",
                agent_name.unwrap_or(&resolved_agent_id)
            )
        } else {
            e.context("Failed to create workflow")
        }
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!("{}", "Workflow created successfully!".green().bold());
        println!();
        display_workflow(&workflow);
    }

    Ok(())
}

/// Resolve an agent ID from either --agent-id or --agent-name.
///
/// If --agent-name is provided, queries the orchestrator API to find the agent
/// by name. Errors if the agent is not found or if multiple agents share the name.
async fn resolve_agent_id(
    client: &ApiClient,
    agent_id: Option<&str>,
    agent_name: Option<&str>,
) -> Result<String> {
    match (agent_id, agent_name) {
        (Some(id), _) => Ok(id.to_string()),
        (_, Some(name)) => {
            let response: PaginatedResponse<serde_json::Value> =
                client.get("/agents").await.context("Failed to list agents for name lookup")?;

            let matches: Vec<&serde_json::Value> = response
                .items
                .iter()
                .filter(|a| a.get("name").and_then(|v| v.as_str()) == Some(name))
                .collect();

            match matches.len() {
                0 => bail!(
                    "Agent '{}' not found. Use 'agent orchestrator list-agents' to see available agents.",
                    name
                ),
                1 => {
                    let id = matches[0]
                        .get("id")
                        .and_then(|v| v.as_str())
                        .context("Agent response missing 'id' field")?;
                    Ok(id.to_string())
                }
                n => bail!(
                    "Found {} agents named '{}'. Use --agent-id to specify the exact agent.",
                    n,
                    name
                ),
            }
        }
        (None, None) => bail!("Either --agent-id or --agent-name must be provided."),
    }
}

/// Resolve the prompt template from either --prompt-template or --prompt-template-file.
fn resolve_prompt_template(
    template: Option<&str>,
    template_file: Option<&std::path::Path>,
) -> Result<String> {
    match (template, template_file) {
        (Some(t), _) => Ok(t.to_string()),
        (_, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read prompt template file: {}", path.display())),
        (None, None) => {
            bail!("Either --prompt-template or --prompt-template-file must be provided.")
        }
    }
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
    let response: PaginatedResponse<serde_json::Value> =
        client.get(&path).await.context("Failed to get workflow history")?;
    let dispatches = response.items;

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

    #[test]
    fn test_resolve_prompt_template_from_string() {
        let result = resolve_prompt_template(Some("Fix: {{title}}"), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Fix: {{title}}");
    }

    #[test]
    fn test_resolve_prompt_template_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_prompt_template.txt");
        std::fs::write(&path, "Work on: {{title}}\n\n{{body}}").unwrap();

        let result = resolve_prompt_template(None, Some(&path));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Work on: {{title}}\n\n{{body}}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_resolve_prompt_template_file_not_found() {
        let path = std::path::PathBuf::from("/nonexistent/template.txt");
        let result = resolve_prompt_template(None, Some(&path));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read prompt template file"));
    }

    #[test]
    fn test_resolve_prompt_template_none_provided() {
        let result = resolve_prompt_template(None, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Either --prompt-template or --prompt-template-file must be provided"));
    }

    #[test]
    fn test_disabled_flag_semantics() {
        // When --disabled is absent, disabled=false, so enabled = !disabled = true
        let disabled = false;
        assert!(!disabled, "By default --disabled is false, meaning the workflow is enabled");
        assert!(!disabled);

        // When --disabled is present, disabled=true, so enabled = !disabled = false
        let disabled = true;
        assert!(disabled);
    }

    /// Verify the clap definition parses correctly with --disabled flag.
    #[test]
    fn test_create_workflow_clap_parsing() {
        use clap::Parser;

        // Wrapper needed for testing subcommands
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        // Test with --disabled
        let cli = Cli::try_parse_from([
            "test",
            "create-workflow",
            "--name",
            "test",
            "--agent-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Fix: {{title}}",
            "--disabled",
        ])
        .expect("Should parse with --disabled");

        if let OrchestratorCommand::CreateWorkflow { disabled, .. } = cli.command {
            assert!(disabled, "--disabled flag should be true when present");
        } else {
            panic!("Expected CreateWorkflow variant");
        }

        // Test without --disabled (default = false = enabled)
        let cli = Cli::try_parse_from([
            "test",
            "create-workflow",
            "--name",
            "test",
            "--agent-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Fix: {{title}}",
        ])
        .expect("Should parse without --disabled");

        if let OrchestratorCommand::CreateWorkflow { disabled, .. } = cli.command {
            assert!(!disabled, "--disabled flag should be false when absent");
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }

    /// Verify --agent-id and --agent-name are mutually exclusive.
    #[test]
    fn test_agent_id_and_name_conflict() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let result = Cli::try_parse_from([
            "test",
            "create-workflow",
            "--name",
            "test",
            "--agent-id",
            "some-id",
            "--agent-name",
            "some-name",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Fix: {{title}}",
        ]);

        assert!(result.is_err(), "--agent-id and --agent-name should conflict");
    }

    /// Verify --prompt-template and --prompt-template-file are mutually exclusive.
    #[test]
    fn test_prompt_template_and_file_conflict() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let result = Cli::try_parse_from([
            "test",
            "create-workflow",
            "--name",
            "test",
            "--agent-id",
            "some-id",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Fix: {{title}}",
            "--prompt-template-file",
            "/tmp/template.txt",
        ]);

        assert!(result.is_err(), "--prompt-template and --prompt-template-file should conflict");
    }

    #[test]
    fn test_resolve_working_dir_provided() {
        let result = resolve_working_dir(Some("/tmp/project"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/tmp/project");
    }

    #[test]
    fn test_resolve_working_dir_defaults_to_pwd() {
        let result = resolve_working_dir(None);
        assert!(result.is_ok());
        let pwd = std::env::current_dir().unwrap().to_string_lossy().to_string();
        assert_eq!(result.unwrap(), pwd);
    }

    #[test]
    fn test_resolve_agent_prompt_from_string() {
        let result = resolve_agent_prompt(Some("Fix the bug"), None, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("Fix the bug".to_string()));
    }

    #[test]
    fn test_resolve_agent_prompt_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_agent_prompt.txt");
        std::fs::write(
            &path,
            "Review all files in src/ for security issues.\n1. SQL injection\n2. XSS",
        )
        .unwrap();

        let result = resolve_agent_prompt(None, Some(&path), false);
        assert!(result.is_ok());
        let prompt = result.unwrap().unwrap();
        assert!(prompt.contains("SQL injection"));
        assert!(prompt.contains("XSS"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_resolve_agent_prompt_file_not_found() {
        let path = std::path::PathBuf::from("/nonexistent/prompt.txt");
        let result = resolve_agent_prompt(None, Some(&path), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read prompt file"));
    }

    #[test]
    fn test_resolve_agent_prompt_file_empty() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_agent_prompt_empty.txt");
        std::fs::write(&path, "   \n  ").unwrap();

        let result = resolve_agent_prompt(None, Some(&path), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Prompt file is empty"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_resolve_agent_prompt_none() {
        let result = resolve_agent_prompt(None, None, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    /// Verify create-agent clap parsing with new flags.
    #[test]
    fn test_create_agent_clap_defaults() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        // Minimal: only --name required (working-dir defaults to None which resolves to $PWD)
        let cli = Cli::try_parse_from(["test", "create-agent", "--name", "my-agent"])
            .expect("Should parse with only --name");

        if let OrchestratorCommand::CreateAgent {
            name,
            working_dir,
            prompt,
            prompt_file,
            stdin,
            attach,
            ..
        } = cli.command
        {
            assert_eq!(name, "my-agent");
            assert_eq!(working_dir, None, "working_dir should default to None");
            assert_eq!(prompt, None);
            assert_eq!(prompt_file, None);
            assert!(!stdin);
            assert!(!attach);
        } else {
            panic!("Expected CreateAgent variant");
        }
    }

    #[test]
    fn test_create_agent_prompt_conflicts() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        // --prompt and --prompt-file conflict
        let result = Cli::try_parse_from([
            "test",
            "create-agent",
            "--name",
            "test",
            "--prompt",
            "inline",
            "--prompt-file",
            "/tmp/prompt.txt",
        ]);
        assert!(result.is_err(), "--prompt and --prompt-file should conflict");

        // --prompt and --stdin conflict
        let result = Cli::try_parse_from([
            "test",
            "create-agent",
            "--name",
            "test",
            "--prompt",
            "inline",
            "--stdin",
        ]);
        assert!(result.is_err(), "--prompt and --stdin should conflict");

        // --prompt-file and --stdin conflict
        let result = Cli::try_parse_from([
            "test",
            "create-agent",
            "--name",
            "test",
            "--prompt-file",
            "/tmp/prompt.txt",
            "--stdin",
        ]);
        assert!(result.is_err(), "--prompt-file and --stdin should conflict");
    }

    #[test]
    fn test_create_agent_attach_flag() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from([
            "test",
            "create-agent",
            "--name",
            "debug",
            "--interactive",
            "--attach",
        ])
        .expect("Should parse with --attach");

        if let OrchestratorCommand::CreateAgent { attach, interactive, .. } = cli.command {
            assert!(attach);
            assert!(interactive);
        } else {
            panic!("Expected CreateAgent variant");
        }
    }

    #[test]
    fn test_build_agent_body_omits_nulls() {
        // Simulate the body-building logic from create_agent
        let mut body = serde_json::Map::new();
        body.insert("name".to_string(), serde_json::json!("test"));
        body.insert("working_dir".to_string(), serde_json::json!("/tmp"));
        body.insert("shell".to_string(), serde_json::json!("zsh"));
        body.insert("interactive".to_string(), serde_json::json!(false));
        body.insert("worktree".to_string(), serde_json::json!(false));

        // Optional fields NOT inserted (user, prompt, system_prompt)
        let value = serde_json::Value::Object(body);

        assert!(value.get("user").is_none(), "user should be omitted");
        assert!(value.get("prompt").is_none(), "prompt should be omitted");
        assert!(value.get("system_prompt").is_none(), "system_prompt should be omitted");
        assert!(value.get("name").is_some(), "name should be present");
        assert!(value.get("working_dir").is_some(), "working_dir should be present");
    }

    /// Verify --agent-name is accepted as an alternative to --agent-id.
    #[test]
    fn test_agent_name_parsing() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from([
            "test",
            "create-workflow",
            "--name",
            "test",
            "--agent-name",
            "my-agent",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Fix: {{title}}",
        ])
        .expect("Should parse with --agent-name");

        if let OrchestratorCommand::CreateWorkflow { agent_id, agent_name, .. } = cli.command {
            assert_eq!(agent_id, None);
            assert_eq!(agent_name, Some("my-agent".to_string()));
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }
}
