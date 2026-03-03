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
use futures_util::StreamExt;
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

    /// Stream real-time output from agents via WebSocket.
    ///
    /// Connects to the orchestrator's monitoring WebSocket endpoint and
    /// displays agent output with formatted, colored messages.
    ///
    /// # Examples
    ///
    /// Watch output from a specific agent:
    ///
    ///   agent orchestrator stream 550e8400-e29b-41d4-a716-446655440000
    ///
    /// Watch output from all agents:
    ///
    ///   agent orchestrator stream --all
    ///
    /// Stream raw JSON (for piping):
    ///
    ///   agent orchestrator stream --all --json
    Stream {
        /// Agent ID to stream (omit for --all)
        #[arg(conflicts_with = "all")]
        id: Option<String>,

        /// Stream output from all connected agents
        #[arg(long, conflicts_with = "id")]
        all: bool,

        /// Show keep-alive and debug messages
        #[arg(long)]
        verbose: bool,
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
            OrchestratorCommand::Stream { id, all, verbose } => {
                stream_agents(client, id.as_deref(), *all, *verbose, json).await
            }
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
            let response: PaginatedResponse<serde_json::Value> = client
                .get("/agents")
                .await
                .context("Failed to list agents for name lookup")?;

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
        (_, Some(path)) => {
            std::fs::read_to_string(path).with_context(|| {
                format!("Failed to read prompt template file: {}", path.display())
            })
        }
        (None, None) => bail!("Either --prompt-template or --prompt-template-file must be provided."),
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

// -- Stream --

/// Stream real-time agent output via WebSocket.
///
/// Connects to the orchestrator's monitoring WebSocket and displays
/// formatted messages until the user presses Ctrl+C.
async fn stream_agents(
    _client: &ApiClient,
    id: Option<&str>,
    all: bool,
    verbose: bool,
    json: bool,
) -> Result<()> {
    if id.is_none() && !all {
        bail!("Either an agent ID or --all must be provided.");
    }

    // Build the WebSocket URL from the orchestrator's base URL
    let base_url = std::env::var("ORCHESTRATOR_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:7006".to_string());

    let ws_base = base_url
        .replace("http://", "ws://")
        .replace("https://", "wss://");

    let ws_url = match id {
        Some(agent_id) => format!("{}/stream/{}", ws_base, agent_id),
        None => format!("{}/stream", ws_base),
    };

    if !json {
        let target = match id {
            Some(agent_id) => format!("agent {}", agent_id),
            None => "all agents".to_string(),
        };
        eprintln!(
            "{}",
            format!("Connecting to stream ({})...", target).cyan()
        );
        eprintln!(
            "{}",
            "Press Ctrl+C to disconnect.".bright_black()
        );
        eprintln!();
    }

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .context("Failed to connect to orchestrator WebSocket. Is the orchestrator running?")?;

    let (_, mut read) = ws_stream.split();

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        if json {
                            println!("{}", text);
                        } else {
                            format_stream_message(&text, verbose);
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                        if !json {
                            eprintln!("{}", "Stream closed by server.".yellow());
                        }
                        break;
                    }
                    Some(Err(e)) => {
                        if !json {
                            eprintln!("{}", format!("WebSocket error: {}", e).red());
                        }
                        break;
                    }
                    None => {
                        if !json {
                            eprintln!("{}", "Stream ended.".yellow());
                        }
                        break;
                    }
                    _ => {} // Ping/Pong/Binary — ignore
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if !json {
                    eprintln!();
                    eprintln!("{}", "Disconnected.".yellow());
                }
                break;
            }
        }
    }

    Ok(())
}

/// Format and display a single stream-JSON message with colors.
fn format_stream_message(text: &str, verbose: bool) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            println!("{}", text.bright_black());
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let agent_id = msg
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let agent_short = if agent_id.len() > 8 {
        &agent_id[..8]
    } else {
        agent_id
    };
    let prefix = format!("[{}]", agent_short).bright_black();

    match msg_type {
        "assistant" => {
            // Extract content from the message
            if let Some(message) = msg.get("message") {
                if let Some(content) = message.get("content") {
                    // Content can be a string or an array of content blocks
                    if let Some(text) = content.as_str() {
                        for line in text.lines() {
                            println!("{} {}", prefix, line);
                        }
                    } else if let Some(blocks) = content.as_array() {
                        for block in blocks {
                            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                for line in text.lines() {
                                    println!("{} {}", prefix, line);
                                }
                            } else if block.get("type").and_then(|v| v.as_str())
                                == Some("tool_use")
                            {
                                let tool = block
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                println!(
                                    "{} {}",
                                    prefix,
                                    format!("🔧 Using tool: {}", tool).yellow()
                                );
                            }
                        }
                    }
                }
            }
        }
        "result" => {
            let is_error = msg.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
            let result_text = msg
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_error {
                println!(
                    "{} {}",
                    prefix,
                    format!("❌ Error: {}", result_text).red()
                );
            } else {
                let display = if result_text.is_empty() {
                    "✅ Task completed".to_string()
                } else if result_text.len() > 120 {
                    format!("✅ {}", &result_text[..117])
                } else {
                    format!("✅ {}", result_text)
                };
                println!("{} {}", prefix, display.green());
            }
        }
        "system" => {
            if verbose {
                let subtype = msg.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                println!(
                    "{} {}",
                    prefix,
                    format!("[system:{}]", subtype).bright_black()
                );
            }
        }
        "control_request" => {
            if let Some(request) = msg.get("request") {
                let tool_name = request
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!(
                    "{} {}",
                    prefix,
                    format!("⚡ Permission request: {}", tool_name).yellow()
                );
            }
        }
        "keep_alive" => {
            if verbose {
                println!("{} {}", prefix, "♥ keep-alive".bright_black());
            }
        }
        _ => {
            if verbose {
                println!(
                    "{} {}",
                    prefix,
                    format!("[{}] {}", msg_type, text).bright_black()
                );
            }
        }
    }
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
        assert!(
            !disabled,
            "By default --disabled is false, meaning the workflow is enabled"
        );
        assert_eq!(!disabled, true);

        // When --disabled is present, disabled=true, so enabled = !disabled = false
        let disabled = true;
        assert_eq!(!disabled, false);
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
            "--name", "test",
            "--agent-id", "550e8400-e29b-41d4-a716-446655440000",
            "--owner", "acme",
            "--repo", "widgets",
            "--prompt-template", "Fix: {{title}}",
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
            "--name", "test",
            "--agent-id", "550e8400-e29b-41d4-a716-446655440000",
            "--owner", "acme",
            "--repo", "widgets",
            "--prompt-template", "Fix: {{title}}",
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
            "--name", "test",
            "--agent-id", "some-id",
            "--agent-name", "some-name",
            "--owner", "acme",
            "--repo", "widgets",
            "--prompt-template", "Fix: {{title}}",
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
            "--name", "test",
            "--agent-id", "some-id",
            "--owner", "acme",
            "--repo", "widgets",
            "--prompt-template", "Fix: {{title}}",
            "--prompt-template-file", "/tmp/template.txt",
        ]);

        assert!(
            result.is_err(),
            "--prompt-template and --prompt-template-file should conflict"
        );
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
            "--name", "test",
            "--agent-name", "my-agent",
            "--owner", "acme",
            "--repo", "widgets",
            "--prompt-template", "Fix: {{title}}",
        ])
        .expect("Should parse with --agent-name");

        if let OrchestratorCommand::CreateWorkflow {
            agent_id,
            agent_name,
            ..
        } = cli.command
        {
            assert_eq!(agent_id, None);
            assert_eq!(agent_name, Some("my-agent".to_string()));
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }

    /// Verify stream subcommand parses with agent ID.
    #[test]
    fn test_stream_by_id() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from([
            "test",
            "stream",
            "550e8400-e29b-41d4-a716-446655440000",
        ])
        .expect("Should parse stream with ID");

        if let OrchestratorCommand::Stream { id, all, verbose } = cli.command {
            assert_eq!(
                id,
                Some("550e8400-e29b-41d4-a716-446655440000".to_string())
            );
            assert!(!all);
            assert!(!verbose);
        } else {
            panic!("Expected Stream variant");
        }
    }

    /// Verify stream --all parses correctly.
    #[test]
    fn test_stream_all() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from(["test", "stream", "--all"])
            .expect("Should parse stream --all");

        if let OrchestratorCommand::Stream { id, all, .. } = cli.command {
            assert_eq!(id, None);
            assert!(all);
        } else {
            panic!("Expected Stream variant");
        }
    }

    /// Verify stream ID and --all conflict.
    #[test]
    fn test_stream_id_and_all_conflict() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let result = Cli::try_parse_from([
            "test",
            "stream",
            "550e8400-e29b-41d4-a716-446655440000",
            "--all",
        ]);

        assert!(result.is_err(), "ID and --all should conflict");
    }

    /// Verify format_stream_message handles assistant messages.
    #[test]
    fn test_format_assistant_message() {
        let msg = serde_json::json!({
            "type": "assistant",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "message": {
                "role": "assistant",
                "content": "I'll analyze the code."
            }
        });
        // Should not panic
        format_stream_message(&serde_json::to_string(&msg).unwrap(), false);
    }

    /// Verify format_stream_message handles result messages.
    #[test]
    fn test_format_result_message() {
        let msg = serde_json::json!({
            "type": "result",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "is_error": false,
            "result": "Fixed the bug in main.rs"
        });
        // Should not panic
        format_stream_message(&serde_json::to_string(&msg).unwrap(), false);
    }

    /// Verify format_stream_message handles error results.
    #[test]
    fn test_format_error_result_message() {
        let msg = serde_json::json!({
            "type": "result",
            "agent_id": "abc12345",
            "is_error": true,
            "result": "Failed to compile"
        });
        format_stream_message(&serde_json::to_string(&msg).unwrap(), false);
    }

    /// Verify format_stream_message suppresses keep_alive without verbose.
    #[test]
    fn test_format_keepalive_suppressed() {
        let msg = serde_json::json!({
            "type": "keep_alive",
            "agent_id": "abc12345"
        });
        // Should not print anything (no panic)
        format_stream_message(&serde_json::to_string(&msg).unwrap(), false);
    }

    /// Verify format_stream_message handles tool_use content blocks.
    #[test]
    fn test_format_tool_use_message() {
        let msg = serde_json::json!({
            "type": "assistant",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "message": {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Let me read the file."},
                    {"type": "tool_use", "name": "Read", "input": {"path": "src/main.rs"}}
                ]
            }
        });
        format_stream_message(&serde_json::to_string(&msg).unwrap(), false);
    }

    /// Verify format_stream_message handles invalid JSON gracefully.
    #[test]
    fn test_format_invalid_json() {
        format_stream_message("not valid json {{{", false);
    }
}
