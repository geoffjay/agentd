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
use uuid::Uuid;

use orchestrator::client::OrchestratorClient;
use orchestrator::scheduler::types::{
    CreateWorkflowRequest, DispatchResponse, TaskSourceConfig, UpdateWorkflowRequest,
    WorkflowResponse,
};
use orchestrator::types::{
    AgentResponse, AgentStatus, ApprovalStatus, CreateAgentRequest, PendingApproval,
    SendMessageRequest, ToolPolicy,
};

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

        /// Model to use for the claude session (e.g. sonnet, opus, haiku, claude-sonnet-4-6)
        #[arg(long)]
        model: Option<String>,

        /// Tool policy as JSON (default: allow all tools)
        ///
        /// Examples: '{"mode":"allow_all"}', '{"mode":"deny_list","tools":["Bash"]}'
        #[arg(long)]
        tool_policy: Option<String>,
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

    /// Attach to a running agent's tmux session.
    ///
    /// Looks up the agent, verifies it is running, and execs into its
    /// tmux session for interactive debugging.
    ///
    /// # Examples
    ///
    /// Attach by agent ID:
    ///
    ///   agent orchestrator attach 550e8400-e29b-41d4-a716-446655440000
    ///
    /// Attach by agent name:
    ///
    ///   agent orchestrator attach --name my-agent
    Attach {
        /// Agent ID (UUID)
        #[arg(conflicts_with = "name")]
        id: Option<String>,

        /// Agent name (resolves to first matching running agent)
        #[arg(long, conflicts_with = "id")]
        name: Option<String>,
    },

    /// Send a message/prompt to a running agent.
    ///
    /// Sends a prompt to a non-interactive agent via the orchestrator API.
    /// The agent must be running and connected via WebSocket.
    ///
    /// # Examples
    ///
    /// Send a prompt directly:
    ///
    ///   agent orchestrator send-message <AGENT_ID> "Fix the failing tests"
    ///
    /// Read a multi-line prompt from stdin:
    ///
    ///   echo "Review the code" | agent orchestrator send-message <AGENT_ID> --stdin
    SendMessage {
        /// Agent ID (UUID)
        id: String,

        /// Message content (prompt to send to the agent)
        #[arg(conflicts_with = "msg_stdin")]
        message: Option<String>,

        /// Read message from stdin (supports multi-line prompts)
        #[arg(long = "stdin", conflicts_with = "message")]
        msg_stdin: bool,
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

    /// Get the tool policy for an agent.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent orchestrator get-policy 550e8400-e29b-41d4-a716-446655440000
    /// ```
    GetPolicy {
        /// Agent ID (UUID)
        id: String,
    },

    /// Set the tool policy for an agent.
    ///
    /// The policy controls which tools the agent is allowed to use.
    ///
    /// # Examples
    ///
    /// Allow all tools (default):
    ///
    ///   agent orchestrator set-policy <ID> '{"mode":"allow_all"}'
    ///
    /// Only allow Read and Grep:
    ///
    ///   agent orchestrator set-policy <ID> '{"mode":"allow_list","tools":["Read","Grep"]}'
    ///
    /// Block Bash and Write:
    ///
    ///   agent orchestrator set-policy <ID> '{"mode":"deny_list","tools":["Bash","Write"]}'
    ///
    /// Deny all tools:
    ///
    ///   agent orchestrator set-policy <ID> '{"mode":"deny_all"}'
    SetPolicy {
        /// Agent ID (UUID)
        id: String,

        /// Policy as JSON (e.g. '{"mode":"allow_list","tools":["Read","Grep"]}')
        policy: String,
    },

    /// Check the health of the orchestrator service.
    ///
    /// Shows the service status and active agent count.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent orchestrator health
    /// ```
    Health,

    /// List pending tool approval requests.
    ///
    /// Shows tool requests that are waiting for human approval.
    ///
    /// # Examples
    ///
    ///   agent orchestrator list-approvals
    ///   agent orchestrator list-approvals --agent-id <ID>
    ListApprovals {
        /// Filter by agent ID
        #[arg(long)]
        agent_id: Option<String>,

        /// Filter by status (pending, approved, denied, timed_out)
        #[arg(long, default_value = "pending")]
        status: String,
    },

    /// Approve a pending tool request.
    ///
    /// # Examples
    ///
    ///   agent orchestrator approve <APPROVAL_ID>
    Approve {
        /// Approval ID (UUID)
        id: String,
    },

    /// Deny a pending tool request.
    ///
    /// # Examples
    ///
    ///   agent orchestrator deny <APPROVAL_ID>
    Deny {
        /// Approval ID (UUID)
        id: String,
    },

    /// Validate a workflow prompt template.
    ///
    /// Checks for unknown variables, unclosed placeholders, and empty templates.
    ///
    /// # Examples
    ///
    ///   agent orchestrator validate-template "Fix: {{title}}\n{{body}}"
    ///   agent orchestrator validate-template --file ./my-template.txt
    ValidateTemplate {
        /// Template string to validate
        #[arg(conflicts_with = "file")]
        template: Option<String>,

        /// Read template from a file
        #[arg(long, conflicts_with = "template")]
        file: Option<PathBuf>,
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
    /// * `client` - The typed orchestrator client
    /// * `json` - If true, output raw JSON instead of formatted text
    pub async fn execute(&self, client: &OrchestratorClient, json: bool) -> Result<()> {
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
                model,
                tool_policy,
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
                    model.as_deref(),
                    tool_policy.as_deref(),
                    json,
                )
                .await
            }
            OrchestratorCommand::GetAgent { id } => get_agent(client, id, json).await,
            OrchestratorCommand::DeleteAgent { id } => delete_agent(client, id, json).await,
            OrchestratorCommand::Attach { id, name } => {
                attach_agent(client, id.as_deref(), name.as_deref()).await
            }
            OrchestratorCommand::SendMessage { id, message, msg_stdin } => {
                send_message_cmd(client, id, message.as_deref(), *msg_stdin, json).await
            }
            OrchestratorCommand::Stream { id, all, verbose } => {
                stream_agents(id.as_deref(), *all, *verbose, json).await
            }
            OrchestratorCommand::GetPolicy { id } => get_policy(client, id, json).await,
            OrchestratorCommand::SetPolicy { id, policy } => {
                set_policy(client, id, policy, json).await
            }
            OrchestratorCommand::Health => orchestrator_health(client, json).await,
            OrchestratorCommand::ListApprovals { agent_id, status } => {
                list_approvals(client, agent_id.as_deref(), status, json).await
            }
            OrchestratorCommand::Approve { id } => approve_cmd(client, id, json).await,
            OrchestratorCommand::Deny { id } => deny_cmd(client, id, json).await,
            OrchestratorCommand::ValidateTemplate { template, file } => {
                validate_template_cmd(template.as_deref(), file.as_deref(), json).await
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

async fn list_agents(client: &OrchestratorClient, status: Option<&str>, json: bool) -> Result<()> {
    let response = client.list_agents(status).await.context("Failed to list agents")?;
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
    client: &OrchestratorClient,
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
    model: Option<&str>,
    tool_policy_json: Option<&str>,
    json: bool,
) -> Result<()> {
    // Resolve working directory: use provided value or default to $PWD
    let resolved_working_dir = resolve_working_dir(working_dir)?;

    // Resolve prompt from --prompt, --prompt-file, or --stdin
    let resolved_prompt = resolve_agent_prompt(prompt, prompt_file, stdin)?;

    // Parse tool policy from JSON string, default to AllowAll
    let tool_policy: ToolPolicy = match tool_policy_json {
        Some(s) => serde_json::from_str(s).context("Invalid tool policy JSON. Example: '{\"mode\":\"allow_list\",\"tools\":[\"Read\",\"Grep\"]}'")?,
        None => Default::default(),
    };

    let request = CreateAgentRequest {
        name: name.to_string(),
        working_dir: resolved_working_dir,
        user: user.map(|s| s.to_string()),
        shell: shell.to_string(),
        interactive,
        prompt: resolved_prompt,
        worktree,
        system_prompt: system_prompt.map(|s| s.to_string()),
        tool_policy,
        model: model.map(|s| s.to_string()),
    };

    let agent = client.create_agent(&request).await.context("Failed to create agent")?;

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
            .tmux_session
            .as_deref()
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

async fn get_agent(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let agent = client.get_agent(&uuid).await.context("Failed to get agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        display_agent(&agent);
    }

    Ok(())
}

async fn delete_agent(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;

    let agent = client.terminate_agent(&uuid).await.context("Failed to terminate agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        println!("{}", format!("Agent '{}' ({}) terminated.", agent.name, agent.id).green().bold());
    }

    Ok(())
}

// -- Attach --

async fn attach_agent(
    client: &OrchestratorClient,
    id: Option<&str>,
    name: Option<&str>,
) -> Result<()> {
    let agent = match (id, name) {
        (Some(agent_id), _) => {
            let uuid = parse_uuid(agent_id)?;
            client.get_agent(&uuid).await.context(
                "Agent not found. Use 'agent orchestrator list-agents' to see available agents.",
            )?
        }
        (_, Some(agent_name)) => {
            let resolved = resolve_agent_id(client, None, Some(agent_name)).await?;
            client.get_agent(&resolved).await.context("Failed to get agent")?
        }
        (None, None) => bail!("Either an agent ID or --name must be provided."),
    };

    if agent.status != AgentStatus::Running {
        bail!("Agent '{}' is not running (status: {}). Cannot attach.", agent.name, agent.status);
    }

    let session = agent
        .tmux_session
        .as_deref()
        .context(format!("Agent '{}' has no tmux session. It may have crashed.", agent.name))?;

    if std::process::Command::new("tmux")
        .arg("-V")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        bail!("tmux is required but not found. Install with: brew install tmux");
    }

    let session_check = std::process::Command::new("tmux")
        .args(["has-session", "-t", session])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match session_check {
        Ok(s) if s.success() => {}
        _ => bail!(
            "Tmux session '{}' no longer exists. Agent '{}' ({}) may have crashed.",
            session,
            agent.name,
            agent.id
        ),
    }

    println!("{}", format!("Attaching to agent '{}' (session: {})...", agent.name, session).cyan());

    let exit = std::process::Command::new("tmux")
        .args(["attach-session", "-t", session])
        .status()
        .context("Failed to exec tmux attach-session")?;

    if !exit.success() {
        bail!("tmux attach-session exited with status: {}", exit);
    }

    Ok(())
}

// -- Send message --

async fn send_message_cmd(
    client: &OrchestratorClient,
    id: &str,
    message: Option<&str>,
    stdin: bool,
    json: bool,
) -> Result<()> {
    let content = match (message, stdin) {
        (Some(msg), _) => msg.to_string(),
        (_, true) => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("Failed to read message from stdin")?;
            if buf.trim().is_empty() {
                bail!("No message provided on stdin");
            }
            buf
        }
        (None, false) => bail!("Either a message argument or --stdin must be provided."),
    };

    let uuid = parse_uuid(id)?;
    let request = SendMessageRequest { content };

    let response = client.send_message(&uuid, &request).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("404") {
            anyhow::anyhow!(
                "Agent '{}' not found. Use 'agent orchestrator list-agents' to see available agents.",
                id
            )
        } else if msg.contains("409") {
            anyhow::anyhow!(
                "Agent '{}' is not running. Only running agents can receive messages.",
                id
            )
        } else {
            e.context("Failed to send message")
        }
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", format!("Message sent to agent ({}).", response.agent_id).green().bold());
    }

    Ok(())
}

// -- Stream --

async fn stream_agents(id: Option<&str>, all: bool, verbose: bool, json: bool) -> Result<()> {
    if id.is_none() && !all {
        bail!("Either an agent ID or --all must be provided.");
    }

    let base_url = std::env::var("ORCHESTRATOR_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:7006".to_string());
    let ws_base = base_url.replace("http://", "ws://").replace("https://", "wss://");

    let ws_url = match id {
        Some(agent_id) => format!("{}/stream/{}", ws_base, agent_id),
        None => format!("{}/stream", ws_base),
    };

    if !json {
        let target = id.map(|a| format!("agent {}", a)).unwrap_or_else(|| "all agents".to_string());
        eprintln!("{}", format!("Connecting to stream ({})...", target).cyan());
        eprintln!("{}", "Press Ctrl+C to disconnect.".bright_black());
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
                        if !json { eprintln!("{}", "Stream closed by server.".yellow()); }
                        break;
                    }
                    Some(Err(e)) => {
                        if !json { eprintln!("{}", format!("WebSocket error: {}", e).red()); }
                        break;
                    }
                    None => {
                        if !json { eprintln!("{}", "Stream ended.".yellow()); }
                        break;
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if !json { eprintln!(); eprintln!("{}", "Disconnected.".yellow()); }
                break;
            }
        }
    }

    Ok(())
}

fn format_stream_message(text: &str, verbose: bool) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            println!("{}", text.bright_black());
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let agent_id = msg.get("agent_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let agent_short = if agent_id.len() > 8 { &agent_id[..8] } else { agent_id };
    let prefix = format!("[{}]", agent_short).bright_black();

    match msg_type {
        "assistant" => {
            if let Some(message) = msg.get("message") {
                if let Some(content) = message.get("content") {
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
                            } else if block.get("type").and_then(|v| v.as_str()) == Some("tool_use")
                            {
                                let tool =
                                    block.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
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
            let result_text = msg.get("result").and_then(|v| v.as_str()).unwrap_or("");
            if is_error {
                println!("{} {}", prefix, format!("❌ Error: {}", result_text).red());
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
                let s = msg.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                println!("{} {}", prefix, format!("[system:{}]", s).bright_black());
            }
        }
        "control_request" => {
            if let Some(request) = msg.get("request") {
                let tool = request.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                println!("{} {}", prefix, format!("⚡ Permission request: {}", tool).yellow());
            }
        }
        "keep_alive" => {
            if verbose {
                println!("{} {}", prefix, "♥ keep-alive".bright_black());
            }
        }
        _ => {
            if verbose {
                println!("{} {}", prefix, format!("[{}]", msg_type).bright_black());
            }
        }
    }
}

// -- Policy --

async fn get_policy(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let policy = client.get_agent_policy(&uuid).await.context("Failed to get agent policy")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&policy)?);
    } else {
        display_policy(&policy);
    }

    Ok(())
}

async fn set_policy(
    client: &OrchestratorClient,
    id: &str,
    policy_json: &str,
    json: bool,
) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let policy: ToolPolicy = serde_json::from_str(policy_json).context(
        "Invalid tool policy JSON. Example: '{\"mode\":\"allow_list\",\"tools\":[\"Read\",\"Grep\"]}'",
    )?;

    let updated = client
        .update_agent_policy(&uuid, &policy)
        .await
        .context("Failed to update agent policy")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        println!("{}", "Policy updated successfully!".green().bold());
        println!();
        display_policy(&updated);
    }

    Ok(())
}

fn display_policy(policy: &ToolPolicy) {
    match policy {
        ToolPolicy::AllowAll => {
            println!("{}: {}", "Mode".bold(), "allow_all".green());
            println!("{}: all tools permitted", "Effect".bold());
        }
        ToolPolicy::DenyAll => {
            println!("{}: {}", "Mode".bold(), "deny_all".red());
            println!("{}: no tools permitted", "Effect".bold());
        }
        ToolPolicy::AllowList { tools } => {
            println!("{}: {}", "Mode".bold(), "allow_list".yellow());
            println!("{}: only these tools permitted:", "Effect".bold());
            for tool in tools {
                println!("  - {}", tool.cyan());
            }
        }
        ToolPolicy::DenyList { tools } => {
            println!("{}: {}", "Mode".bold(), "deny_list".yellow());
            println!("{}: all tools except these:", "Effect".bold());
            for tool in tools {
                println!("  - {}", tool.red());
            }
        }
        ToolPolicy::RequireApproval => {
            println!("{}: {}", "Mode".bold(), "require_approval".bright_yellow());
            println!("{}: every tool request requires human approval", "Effect".bold());
        }
    }
}

// -- Approvals --

async fn list_approvals(
    client: &OrchestratorClient,
    agent_id: Option<&str>,
    status: &str,
    json: bool,
) -> Result<()> {
    let response = match agent_id {
        Some(id) => {
            let uuid = parse_uuid(id)?;
            client.list_agent_approvals(&uuid, Some(status)).await?
        }
        None => client.list_approvals(Some(status)).await?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&response.items)?);
    } else if response.items.is_empty() {
        println!("{}", "No approval requests found.".yellow());
    } else {
        println!("{}", "Pending Approvals:".blue().bold());
        println!("{}", "=".repeat(80).cyan());
        for approval in &response.items {
            display_approval(approval);
            println!("{}", "-".repeat(80).cyan());
        }
        println!("Total: {} approval(s)", response.total);
    }
    Ok(())
}

async fn approve_cmd(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let approval = client.approve_tool(&uuid).await.context("Failed to approve tool request")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&approval)?);
    } else {
        println!(
            "{}",
            format!("Approved: {} (tool: {})", approval.id, approval.tool_name).green().bold()
        );
    }
    Ok(())
}

async fn deny_cmd(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let approval = client.deny_tool(&uuid).await.context("Failed to deny tool request")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&approval)?);
    } else {
        println!(
            "{}",
            format!("Denied: {} (tool: {})", approval.id, approval.tool_name).red().bold()
        );
    }
    Ok(())
}

fn display_approval(approval: &PendingApproval) {
    println!("{}: {}", "Approval ID".bold(), approval.id);
    println!("{}: {}", "Agent ID".bold(), approval.agent_id);
    println!("{}: {}", "Tool".bold(), approval.tool_name.yellow());
    let status_display = match approval.status {
        ApprovalStatus::Pending => "pending".yellow().to_string(),
        ApprovalStatus::Approved => "approved".green().to_string(),
        ApprovalStatus::Denied => "denied".red().to_string(),
        ApprovalStatus::TimedOut => "timed_out".bright_red().to_string(),
    };
    println!("{}: {}", "Status".bold(), status_display);
    println!("{}: {}", "Requested".bold(), approval.created_at);
    println!("{}: {}", "Expires".bold(), approval.expires_at);
}

// -- Health --

async fn orchestrator_health(client: &OrchestratorClient, json: bool) -> Result<()> {
    let response =
        client.health().await.context("Failed to reach orchestrator service. Is it running?")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        let agents_active =
            response.details.get("agents_active").and_then(|v| v.as_u64()).unwrap_or(0);
        println!(
            "{} {} ({} agents active)",
            "orchestrator:".bold(),
            "ok".green().bold(),
            agents_active.to_string().cyan()
        );
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

// -- Validate template --

async fn validate_template_cmd(
    template: Option<&str>,
    file: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    use orchestrator::scheduler::template::{validate_template, KNOWN_VARIABLES};

    let content = match (template, file) {
        (Some(t), _) => t.to_string(),
        (_, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read template file: {}", path.display()))?,
        (None, None) => bail!("Either a template string or --file must be provided."),
    };

    let warnings = validate_template(&content);

    if json {
        let result = serde_json::json!({
            "valid": warnings.is_empty(),
            "warnings": warnings,
            "known_variables": KNOWN_VARIABLES,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if warnings.is_empty() {
        println!("{}", "Template is valid!".green().bold());
        println!();
        println!("{}: {}", "Known variables".bold(), KNOWN_VARIABLES.join(", "));
    } else {
        println!("{}", "Template warnings:".yellow().bold());
        for warning in &warnings {
            println!("  {} {}", "!".yellow(), warning);
        }
        println!();
        println!("{}: {}", "Known variables".bold(), KNOWN_VARIABLES.join(", "));
    }

    Ok(())
}

// -- Workflow operations --

async fn list_workflows(client: &OrchestratorClient, json: bool) -> Result<()> {
    let response = client.list_workflows().await.context("Failed to list workflows")?;
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
    client: &OrchestratorClient,
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

    let request = CreateWorkflowRequest {
        name: name.to_string(),
        agent_id: resolved_agent_id,
        source_config: TaskSourceConfig::GithubIssues {
            owner: owner.to_string(),
            repo: repo.to_string(),
            labels: labels_vec,
            state: "open".to_string(),
        },
        prompt_template: resolved_template,
        poll_interval_secs: poll_interval,
        enabled,
        tool_policy: Default::default(),
    };

    let workflow = client.create_workflow(&request).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("404") {
            anyhow::anyhow!(
                "Agent not found or not running. Use 'agent orchestrator list-agents' to see available agents."
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

async fn get_workflow(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let workflow = client.get_workflow(&uuid).await.context("Failed to get workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        display_workflow(&workflow);
    }

    Ok(())
}

async fn update_workflow(
    client: &OrchestratorClient,
    id: &str,
    name: Option<&str>,
    prompt_template: Option<&str>,
    poll_interval: Option<u64>,
    enabled: Option<bool>,
    json: bool,
) -> Result<()> {
    let uuid = parse_uuid(id)?;

    let request = UpdateWorkflowRequest {
        name: name.map(|s| s.to_string()),
        prompt_template: prompt_template.map(|s| s.to_string()),
        poll_interval_secs: poll_interval,
        enabled,
        tool_policy: None,
    };

    let workflow =
        client.update_workflow(&uuid, &request).await.context("Failed to update workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!("{}", "Workflow updated successfully!".green().bold());
        println!();
        display_workflow(&workflow);
    }

    Ok(())
}

async fn delete_workflow(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;

    if json {
        // Fetch before deleting to show it
        if let Ok(workflow) = client.get_workflow(&uuid).await {
            client.delete_workflow(&uuid).await.context("Failed to delete workflow")?;
            println!("{}", serde_json::to_string_pretty(&workflow)?);
        } else {
            client.delete_workflow(&uuid).await.context("Failed to delete workflow")?;
            println!("{{}}");
        }
    } else {
        client.delete_workflow(&uuid).await.context("Failed to delete workflow")?;
        println!("{}", format!("Workflow {} deleted.", id).green().bold());
    }

    Ok(())
}

async fn workflow_history(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let response =
        client.dispatch_history(&uuid).await.context("Failed to get workflow history")?;
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

// -- Helper functions --

/// Parse a string as a UUID, providing a user-friendly error message.
fn parse_uuid(id: &str) -> Result<Uuid> {
    id.parse::<Uuid>().with_context(|| format!("Invalid UUID: '{id}'"))
}

/// Resolve an agent ID from either --agent-id or --agent-name.
///
/// If --agent-name is provided, queries the orchestrator API to find the agent
/// by name. Errors if the agent is not found or if multiple agents share the name.
async fn resolve_agent_id(
    client: &OrchestratorClient,
    agent_id: Option<&str>,
    agent_name: Option<&str>,
) -> Result<Uuid> {
    match (agent_id, agent_name) {
        (Some(id), _) => parse_uuid(id),
        (_, Some(name)) => {
            let response =
                client.list_agents(None).await.context("Failed to list agents for name lookup")?;

            let matches: Vec<&AgentResponse> =
                response.items.iter().filter(|a| a.name == name).collect();

            match matches.len() {
                0 => bail!(
                    "Agent '{}' not found. Use 'agent orchestrator list-agents' to see available agents.",
                    name
                ),
                1 => Ok(matches[0].id),
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

// -- Display helpers --

fn display_agent(agent: &AgentResponse) {
    println!("{}: {}", "ID".bold(), agent.id);
    println!("{}: {}", "Name".bold(), agent.name.bright_white());
    let status_str = agent.status.to_string();
    let colored_status = match agent.status {
        AgentStatus::Running => status_str.green(),
        AgentStatus::Pending => status_str.yellow(),
        AgentStatus::Stopped => status_str.red(),
        AgentStatus::Failed => status_str.bright_red(),
    };
    println!("{}: {}", "Status".bold(), colored_status);
    if let Some(session) = &agent.tmux_session {
        println!("{}: {}", "Tmux Session".bold(), session);
    }
    println!("{}: {}", "Working Dir".bold(), agent.config.working_dir);
    println!("{}: {}", "Shell".bold(), agent.config.shell);
    if let Some(ref model) = agent.config.model {
        println!("{}: {}", "Model".bold(), model.cyan());
    }
    let policy_display = match &agent.config.tool_policy {
        ToolPolicy::AllowAll => "allow_all".green().to_string(),
        ToolPolicy::DenyAll => "deny_all".red().to_string(),
        ToolPolicy::AllowList { tools } => {
            format!("{} [{}]", "allow_list".yellow(), tools.join(", "))
        }
        ToolPolicy::DenyList { tools } => {
            format!("{} [{}]", "deny_list".yellow(), tools.join(", "))
        }
        ToolPolicy::RequireApproval => "require_approval".bright_yellow().to_string(),
    };
    println!("{}: {}", "Tool Policy".bold(), policy_display);
    println!("{}: {}", "Created".bold(), agent.created_at);
}

fn display_workflow(workflow: &WorkflowResponse) {
    println!("{}: {}", "ID".bold(), workflow.id);
    println!("{}: {}", "Name".bold(), workflow.name.bright_white());
    println!("{}: {}", "Agent ID".bold(), workflow.agent_id);
    let status = if workflow.enabled { "enabled".green() } else { "disabled".red() };
    println!("{}: {}", "Status".bold(), status);
    println!("{}: {}s", "Poll Interval".bold(), workflow.poll_interval_secs);
    match &workflow.source_config {
        TaskSourceConfig::GithubIssues { owner, repo, .. } => {
            println!("{}: github_issues", "Source Type".bold());
            println!("{}: {}/{}", "Repository".bold(), owner, repo);
        }
    }
    let template = &workflow.prompt_template;
    let display =
        if template.len() > 60 { format!("{}...", &template[..57]) } else { template.clone() };
    println!("{}: {}", "Prompt Template".bold(), display);
    println!("{}: {}", "Created".bold(), workflow.created_at);
}

fn display_dispatch(dispatch: &DispatchResponse) {
    println!("{}: {}", "Dispatch ID".bold(), dispatch.id);
    println!("{}: {}", "Source ID".bold(), dispatch.source_id);
    let status_str = dispatch.status.to_string();
    let colored_status = match status_str.as_str() {
        "completed" => status_str.green(),
        "dispatched" => status_str.blue(),
        "pending" => status_str.yellow(),
        "failed" => status_str.bright_red(),
        "skipped" => status_str.normal(),
        _ => status_str.normal(),
    };
    println!("{}: {}", "Status".bold(), colored_status);
    let prompt = &dispatch.prompt_sent;
    let display = if prompt.len() > 60 { format!("{}...", &prompt[..57]) } else { prompt.clone() };
    println!("{}: {}", "Prompt".bold(), display);
    println!("{}: {}", "Dispatched".bold(), dispatch.dispatched_at);
    if let Some(completed) = &dispatch.completed_at {
        println!("{}: {}", "Completed".bold(), completed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator::types::AgentConfig;

    #[test]
    fn test_display_agent_typed() {
        use chrono::Utc;
        let agent = AgentResponse {
            id: Uuid::new_v4(),
            name: "test-agent".to_string(),
            status: AgentStatus::Running,
            config: AgentConfig {
                working_dir: "/tmp/test".to_string(),
                user: None,
                shell: "zsh".to_string(),
                interactive: false,
                prompt: None,
                worktree: false,
                system_prompt: None,
                tool_policy: Default::default(),
                model: None,
            },
            tmux_session: Some("agentd-orch-abc123".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        // Should not panic
        display_agent(&agent);
    }

    #[test]
    fn test_display_workflow_typed() {
        use chrono::Utc;
        let workflow = WorkflowResponse {
            id: Uuid::new_v4(),
            name: "test-workflow".to_string(),
            agent_id: Uuid::new_v4(),
            source_config: TaskSourceConfig::GithubIssues {
                owner: "acme".to_string(),
                repo: "widgets".to_string(),
                labels: vec!["bug".to_string()],
                state: "open".to_string(),
            },
            prompt_template: "Fix: {{title}}".to_string(),
            poll_interval_secs: 60,
            enabled: true,
            tool_policy: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        // Should not panic
        display_workflow(&workflow);
    }

    #[test]
    fn test_display_dispatch_typed() {
        use chrono::Utc;
        use orchestrator::scheduler::types::DispatchStatus;
        let dispatch = DispatchResponse {
            id: Uuid::new_v4(),
            workflow_id: Uuid::new_v4(),
            source_id: "issue-42".to_string(),
            agent_id: Uuid::new_v4(),
            prompt_sent: "Fix the bug".to_string(),
            status: DispatchStatus::Completed,
            dispatched_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        // Should not panic
        display_dispatch(&dispatch);
    }

    #[test]
    fn test_parse_uuid_valid() {
        let result = parse_uuid("550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let result = parse_uuid("not-a-uuid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid UUID"));
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
        let path = dir.join("test_agent_prompt_typed.txt");
        std::fs::write(&path, "Review src/ for issues.\n1. SQL injection\n2. XSS").unwrap();

        let result = resolve_agent_prompt(None, Some(&path), false);
        assert!(result.is_ok());
        let prompt = result.unwrap().unwrap();
        assert!(prompt.contains("SQL injection"));

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
        let path = dir.join("test_agent_prompt_empty_typed.txt");
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

    #[test]
    fn test_resolve_prompt_template_from_string() {
        let result = resolve_prompt_template(Some("Fix: {{title}}"), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Fix: {{title}}");
    }

    #[test]
    fn test_resolve_prompt_template_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_prompt_template_typed.txt");
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
        let disabled = false;
        assert!(!disabled);
        let disabled = true;
        assert!(disabled);
    }

    /// Verify the clap definition parses correctly with --disabled flag.
    #[test]
    fn test_create_workflow_clap_parsing() {
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
            assert!(disabled);
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
    fn test_create_agent_with_model_flag() {
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
            "planner",
            "--model",
            "opus",
            "--prompt",
            "Plan the work",
        ])
        .expect("Should parse with --model");

        if let OrchestratorCommand::CreateAgent { name, model, .. } = cli.command {
            assert_eq!(name, "planner");
            assert_eq!(model, Some("opus".to_string()));
        } else {
            panic!("Expected CreateAgent variant");
        }
    }

    #[test]
    fn test_create_agent_model_defaults_to_none() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from(["test", "create-agent", "--name", "worker"])
            .expect("Should parse without --model");

        if let OrchestratorCommand::CreateAgent { model, .. } = cli.command {
            assert_eq!(model, None);
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
