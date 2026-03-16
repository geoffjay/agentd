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
    CreateWorkflowRequest, DispatchResponse, TriggerConfig, UpdateWorkflowRequest, WorkflowResponse,
};
use orchestrator::types::{
    AddDirResponse, AgentResponse, AgentStatus, AgentUsageStats, ApprovalStatus,
    ClearContextResponse, CreateAgentRequest, PendingApproval, SendMessageRequest, SessionUsage,
    ToolPolicy,
};

/// Trigger type for workflow creation.
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum TriggerType {
    GithubIssues,
    GithubPullRequests,
    Cron,
    Delay,
    Webhook,
    Manual,
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

        /// Model to use for the claude session (e.g. sonnet, opus, haiku, claude-sonnet-4-6)
        #[arg(long)]
        model: Option<String>,

        /// Tool policy as JSON (default: allow all tools)
        ///
        /// Examples: '{"mode":"allow_all"}', '{"mode":"deny_list","tools":["Bash"]}'
        #[arg(long)]
        tool_policy: Option<String>,

        /// Environment variables to set for the agent (KEY=VALUE format).
        ///
        /// Can be specified multiple times for multiple variables.
        /// Commonly used for ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, ANTHROPIC_BASE_URL.
        ///
        /// Examples:
        ///   --env ANTHROPIC_API_KEY=sk-ant-...
        ///   --env ANTHROPIC_BASE_URL=https://custom-proxy.example.com
        ///   --env ANTHROPIC_AUTH_TOKEN=my-token
        ///
        /// Note: values are redacted in API responses to prevent secret leakage.
        // Future: --env-file <path> to load vars from a dotenv-style file.
        #[arg(long = "env", value_name = "KEY=VALUE")]
        env_vars: Vec<String>,

        /// Token threshold for automatic context clearing.
        ///
        /// When the agent's cumulative token usage exceeds this threshold,
        /// the context is automatically cleared and a new session starts.
        /// If not specified, no automatic context clearing is performed.
        #[arg(long)]
        auto_clear_threshold: Option<u64>,

        /// Network policy for Docker-backed agents (internet, isolated, host_network).
        ///
        /// Controls container networking:
        ///   internet     — bridge network with outbound access (default)
        ///   isolated     — bridge network with DNS disabled (no outbound name resolution)
        ///   host_network — host network mode (Linux only)
        ///
        /// Ignored for tmux-backed agents.
        #[arg(long)]
        network_policy: Option<String>,

        /// Custom Docker image for the agent container.
        ///
        /// Overrides the default image set by AGENTD_DOCKER_IMAGE.
        /// Ignored for tmux-backed agents.
        #[arg(long)]
        docker_image: Option<String>,

        /// CPU limit for the Docker container (e.g., 2.0 means two full cores).
        ///
        /// Ignored for tmux-backed agents.
        #[arg(long)]
        cpu_limit: Option<f64>,

        /// Memory limit in megabytes for the Docker container (e.g., 2048 for 2 GiB).
        ///
        /// Ignored for tmux-backed agents.
        #[arg(long)]
        memory_limit: Option<u64>,

        /// Additional volume mount for the Docker container (host:container[:ro]).
        ///
        /// Can be specified multiple times for multiple mounts.
        /// Format: /host/path:/container/path or /host/path:/container/path:ro
        ///
        /// Ignored for tmux-backed agents.
        #[arg(long = "mount", value_name = "HOST:CONTAINER[:ro]")]
        mounts: Vec<String>,

        /// Additional directories the agent can access via Claude Code's --add-dir flag.
        ///
        /// Can be specified multiple times for multiple directories.
        ///
        /// Example:
        ///   --add-dir /path/to/shared/libs --add-dir /opt/configs
        #[arg(long = "add-dir", value_name = "PATH")]
        add_dirs: Vec<String>,
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

    /// View logs from a Docker-backed agent's container.
    ///
    /// Fetches stdout/stderr from the agent's Docker container.
    /// Only works for agents running on the Docker backend.
    ///
    /// # Examples
    ///
    /// View logs by agent ID:
    ///
    ///   agent orchestrator logs 550e8400-e29b-41d4-a716-446655440000
    ///
    /// View logs by agent name:
    ///
    ///   agent orchestrator logs --name my-agent
    ///
    /// Follow logs (tail -f style):
    ///
    ///   agent orchestrator logs --name my-agent --follow
    ///
    /// Show last 50 lines:
    ///
    ///   agent orchestrator logs --name my-agent --tail 50
    Logs {
        /// Agent ID (UUID)
        #[arg(conflicts_with = "name")]
        id: Option<String>,

        /// Agent name (resolves to first matching agent)
        #[arg(long, conflicts_with = "id")]
        name: Option<String>,

        /// Follow log output (stream new lines)
        #[arg(long, short = 'f')]
        follow: bool,

        /// Number of lines to show from the end of the logs
        #[arg(long, default_value = "100")]
        tail: String,
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

    /// Set or change the model for an agent.
    ///
    /// Updates the model in the agent's config. Use --restart to immediately
    /// kill and re-launch the agent with the new model.
    ///
    /// # Examples
    ///
    /// Change model and restart immediately:
    ///
    ///   agent orchestrator set-model <ID> --model opus --restart
    ///
    /// Change model for next restart (no disruption):
    ///
    ///   agent orchestrator set-model <ID> --model sonnet
    ///
    /// Change by agent name:
    ///
    ///   agent orchestrator set-model --name worker --model opus --restart
    ///
    /// Clear model (inherit default):
    ///
    ///   agent orchestrator set-model <ID> --clear
    SetModel {
        /// Agent ID (UUID)
        #[arg(conflicts_with = "name")]
        id: Option<String>,

        /// Agent name (resolves to first matching agent)
        #[arg(long, conflicts_with = "id")]
        name: Option<String>,

        /// Model to use (e.g. sonnet, opus, haiku, claude-sonnet-4-6)
        #[arg(long, conflicts_with = "clear")]
        model: Option<String>,

        /// Clear the model (inherit Claude Code's default)
        #[arg(long, conflicts_with = "model")]
        clear: bool,

        /// Restart the agent process immediately with the new model
        #[arg(long)]
        restart: bool,
    },

    /// Get usage statistics for an agent.
    ///
    /// Shows current session and cumulative usage including tokens,
    /// cost, turns, and duration.
    ///
    /// # Examples
    ///
    /// By agent ID:
    ///
    ///   agent orchestrator usage 550e8400-e29b-41d4-a716-446655440000
    ///
    /// By agent name:
    ///
    ///   agent orchestrator usage --name my-agent
    Usage {
        /// Agent ID (UUID)
        #[arg(conflicts_with = "name")]
        id: Option<String>,

        /// Agent name (resolves to first matching agent)
        #[arg(long, conflicts_with = "id")]
        name: Option<String>,
    },

    /// Clear an agent's context and start a fresh session.
    ///
    /// Terminates the current context, captures usage stats, and restarts
    /// the agent with a clean session.
    ///
    /// # Examples
    ///
    /// By agent ID:
    ///
    ///   agent orchestrator clear-context 550e8400-e29b-41d4-a716-446655440000
    ///
    /// By agent name:
    ///
    ///   agent orchestrator clear-context --name my-agent
    ClearContext {
        /// Agent ID (UUID)
        #[arg(conflicts_with = "name")]
        id: Option<String>,

        /// Agent name (resolves to first matching agent)
        #[arg(long, conflicts_with = "id")]
        name: Option<String>,
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
    /// Creates an autonomous workflow that triggers task dispatch to an agent.
    /// The workflow is enabled by default unless --disabled is specified.
    ///
    /// # Examples
    ///
    /// Create a GitHub Issues workflow (default trigger type):
    ///
    ///   agent orchestrator create-workflow \
    ///     --name issue-worker \
    ///     --agent-name my-agent \
    ///     --owner acme --repo widgets \
    ///     --labels "bug,help wanted" \
    ///     --prompt-template-file ./prompt.txt \
    ///     --poll-interval 120
    ///
    /// Create a GitHub Pull Requests workflow:
    ///
    ///   agent orchestrator create-workflow \
    ///     --name pr-reviewer \
    ///     --agent-name reviewer \
    ///     --trigger-type github-pull-requests \
    ///     --owner acme --repo widgets \
    ///     --prompt-template "Review: {{title}}"
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

        /// Trigger type (default: github-issues)
        #[arg(long, value_enum, default_value = "github-issues")]
        trigger_type: TriggerType,

        /// GitHub repository owner (required for github-issues and github-pull-requests)
        #[arg(long)]
        owner: Option<String>,

        /// GitHub repository name (required for github-issues and github-pull-requests)
        #[arg(long)]
        repo: Option<String>,

        /// Comma-separated labels to filter issues (GitHub triggers only)
        #[arg(long)]
        labels: Option<String>,

        /// Issue/PR state filter (GitHub triggers only, default: open)
        #[arg(long)]
        state: Option<String>,

        /// Cron expression (required for cron trigger type)
        #[arg(long)]
        cron_expression: Option<String>,

        /// ISO 8601 datetime to run at (required for delay trigger type)
        #[arg(long)]
        run_at: Option<String>,

        /// Webhook secret (optional, for webhook trigger type)
        #[arg(long)]
        webhook_secret: Option<String>,

        /// Prompt template with {{placeholders}} (e.g. "Fix: {{title}}\n{{body}}")
        #[arg(long, conflicts_with = "prompt_template_file")]
        prompt_template: Option<String>,

        /// Path to a file containing the prompt template
        #[arg(long, conflicts_with = "prompt_template")]
        prompt_template_file: Option<PathBuf>,

        /// Poll interval in seconds (default: 60, only for poll-based triggers)
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

    /// Add an additional directory to an agent's accessible paths.
    ///
    /// The directory must exist on the local filesystem. The change takes
    /// effect on the next agent restart.
    ///
    /// # Examples
    ///
    ///   agent orchestrator add-dir <AGENT_ID> /path/to/shared/libs
    AddDir {
        /// Agent ID (UUID)
        id: String,
        /// Directory path to add
        path: String,
    },

    /// Remove an additional directory from an agent's accessible paths.
    ///
    /// The change takes effect on the next agent restart.
    ///
    /// # Examples
    ///
    ///   agent orchestrator remove-dir <AGENT_ID> /path/to/shared/libs
    RemoveDir {
        /// Agent ID (UUID)
        id: String,
        /// Directory path to remove
        path: String,
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
                env_vars,
                auto_clear_threshold,
                network_policy,
                docker_image,
                cpu_limit,
                memory_limit,
                mounts,
                add_dirs,
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
                    env_vars,
                    *auto_clear_threshold,
                    network_policy.as_deref(),
                    docker_image.as_deref(),
                    *cpu_limit,
                    *memory_limit,
                    mounts,
                    add_dirs,
                    json,
                )
                .await
            }
            OrchestratorCommand::GetAgent { id } => get_agent(client, id, json).await,
            OrchestratorCommand::DeleteAgent { id } => delete_agent(client, id, json).await,
            OrchestratorCommand::Attach { id, name } => {
                attach_agent(client, id.as_deref(), name.as_deref()).await
            }
            OrchestratorCommand::Logs { id, name, follow, tail } => {
                agent_logs(client, id.as_deref(), name.as_deref(), *follow, tail).await
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
            OrchestratorCommand::SetModel { id, name, model, clear, restart } => {
                set_model_cmd(
                    client,
                    id.as_deref(),
                    name.as_deref(),
                    model.as_deref(),
                    *clear,
                    *restart,
                    json,
                )
                .await
            }
            OrchestratorCommand::Usage { id, name } => {
                usage_cmd(client, id.as_deref(), name.as_deref(), json).await
            }
            OrchestratorCommand::ClearContext { id, name } => {
                clear_context_cmd(client, id.as_deref(), name.as_deref(), json).await
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
                trigger_type,
                owner,
                repo,
                labels,
                state,
                cron_expression,
                run_at,
                webhook_secret,
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
                    trigger_type,
                    owner.as_deref(),
                    repo.as_deref(),
                    labels.as_deref(),
                    state.as_deref(),
                    cron_expression.as_deref(),
                    run_at.as_deref(),
                    webhook_secret.as_deref(),
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
            OrchestratorCommand::AddDir { id, path } => add_dir_cmd(client, id, path, json).await,
            OrchestratorCommand::RemoveDir { id, path } => {
                remove_dir_cmd(client, id, path, json).await
            }
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

/// Parse a slice of `KEY=VALUE` strings into a `HashMap`.
///
/// The first `=` in each string is the delimiter, so values may themselves
/// contain `=` (e.g. `TOKEN=abc=def` → key `TOKEN`, value `abc=def`).
///
/// Returns an error if any entry does not contain `=`.
fn parse_env_vars(raw: &[String]) -> Result<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for entry in raw {
        match entry.split_once('=') {
            Some((key, value)) => {
                map.insert(key.to_string(), value.to_string());
            }
            None => {
                anyhow::bail!(
                    "Invalid --env value {:?}: expected KEY=VALUE format (missing '=')",
                    entry
                );
            }
        }
    }
    Ok(map)
}

/// Parse `--mount` flag values into [`VolumeMount`] structs.
///
/// Expected format: `host_path:container_path` or `host_path:container_path:ro`
fn parse_mount_flags(raw: &[String]) -> Result<Vec<orchestrator::types::VolumeMount>> {
    let mut mounts = Vec::new();
    for entry in raw {
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        match parts.len() {
            2 => {
                mounts.push(orchestrator::types::VolumeMount {
                    host_path: parts[0].to_string(),
                    container_path: parts[1].to_string(),
                    read_only: false,
                });
            }
            3 => {
                let read_only = match parts[2] {
                    "ro" => true,
                    "rw" => false,
                    other => bail!(
                        "Invalid mount mode '{}' in '{}'. Expected 'ro' or 'rw'.",
                        other,
                        entry
                    ),
                };
                mounts.push(orchestrator::types::VolumeMount {
                    host_path: parts[0].to_string(),
                    container_path: parts[1].to_string(),
                    read_only,
                });
            }
            _ => bail!(
                "Invalid --mount value '{}': expected format host:container or host:container:ro",
                entry
            ),
        }
    }
    Ok(mounts)
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
    env_vars: &[String],
    auto_clear_threshold: Option<u64>,
    network_policy: Option<&str>,
    docker_image: Option<&str>,
    cpu_limit: Option<f64>,
    memory_limit: Option<u64>,
    mounts: &[String],
    add_dirs: &[String],
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

    // Parse --env KEY=VALUE pairs into a HashMap.
    // Values containing '=' are handled correctly: only the first '=' is the delimiter.
    // Entries missing '=' produce a clear error.
    let env = parse_env_vars(env_vars)?;

    // Parse network policy string into the enum.
    let parsed_network_policy =
        network_policy.map(|s| s.parse::<wrap::docker::NetworkPolicy>()).transpose().context(
            "Invalid --network-policy value. Valid options: internet, isolated, host_network",
        )?;

    // Parse --mount flags into VolumeMount structs.
    let extra_mounts = parse_mount_flags(mounts)?;

    // Build resource limits from individual flags.
    let resource_limits = if cpu_limit.is_some() || memory_limit.is_some() {
        Some(orchestrator::types::ResourceLimits { cpu_limit, memory_limit_mb: memory_limit })
    } else {
        None
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
        env,
        auto_clear_threshold,
        network_policy: parsed_network_policy,
        docker_image: docker_image.map(|s| s.to_string()),
        extra_mounts: if extra_mounts.is_empty() { None } else { Some(extra_mounts) },
        resource_limits,
        additional_dirs: add_dirs.to_vec(),
    };

    let agent = client.create_agent(&request).await.context("Failed to create agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        println!("{}", "Agent created successfully!".green().bold());
        println!();
        display_agent(&agent);
    }

    // If --attach was requested, exec into the session (tmux or docker)
    if attach {
        let session = agent
            .session_id
            .as_deref()
            .context("Agent response missing 'session_id' field — cannot attach")?;

        let is_docker = agent.backend_type.as_deref() == Some("docker");

        if is_docker {
            println!();
            println!("{}", format!("Attaching to Docker container: {session}").cyan());

            let status = std::process::Command::new("docker")
                .args(["exec", "-it", session, "bash"])
                .status()
                .context("Failed to exec docker exec")?;

            if !status.success() {
                bail!("docker exec exited with status: {status}");
            }
        } else {
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

// -- Additional directory management --

async fn add_dir_cmd(client: &OrchestratorClient, id: &str, path: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let response = client.add_dir(&uuid, path).await.context("Failed to add directory")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        display_add_dir_response(&response);
    }

    Ok(())
}

async fn remove_dir_cmd(
    client: &OrchestratorClient,
    id: &str,
    path: &str,
    json: bool,
) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let response = client.remove_dir(&uuid, path).await.context("Failed to remove directory")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        display_add_dir_response(&response);
    }

    Ok(())
}

fn display_add_dir_response(response: &AddDirResponse) {
    let dirs = if response.additional_dirs.is_empty() {
        "(none)".dimmed().to_string()
    } else {
        response.additional_dirs.join(", ")
    };
    println!("{}: {}", "Agent ID".bold(), response.agent_id);
    println!("{}: {}", "Additional Dirs".bold(), dirs);
    if response.requires_restart {
        println!("{}", "Note: restart the agent for directory changes to take effect.".yellow());
    }
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
        .session_id
        .as_deref()
        .context(format!("Agent '{}' has no session. It may have crashed.", agent.name))?;

    let is_docker = agent.backend_type.as_deref() == Some("docker");

    if is_docker {
        // Docker backend: exec into the container
        if std::process::Command::new("docker")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_err()
        {
            bail!("docker is required but not found. Install Docker Desktop or Docker Engine.");
        }

        // Verify the container is running
        let container_check = std::process::Command::new("docker")
            .args(["inspect", "--format", "{{.State.Running}}", session])
            .output();

        match container_check {
            Ok(output) if String::from_utf8_lossy(&output.stdout).trim() == "true" => {}
            _ => bail!(
                "Docker container '{}' is not running. Agent '{}' ({}) may have crashed.",
                session,
                agent.name,
                agent.id
            ),
        }

        println!(
            "{}",
            format!("Attaching to agent '{}' (container: {})...", agent.name, session).cyan()
        );

        let exit = std::process::Command::new("docker")
            .args(["exec", "-it", session, "bash"])
            .status()
            .context("Failed to exec docker exec")?;

        if !exit.success() {
            bail!("docker exec exited with status: {}", exit);
        }
    } else {
        // Tmux backend: attach to the tmux session
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

        println!(
            "{}",
            format!("Attaching to agent '{}' (session: {})...", agent.name, session).cyan()
        );

        let exit = std::process::Command::new("tmux")
            .args(["attach-session", "-t", session])
            .status()
            .context("Failed to exec tmux attach-session")?;

        if !exit.success() {
            bail!("tmux attach-session exited with status: {}", exit);
        }
    }

    Ok(())
}

// -- Logs --

async fn agent_logs(
    client: &OrchestratorClient,
    id: Option<&str>,
    name: Option<&str>,
    follow: bool,
    tail: &str,
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

    let is_docker = agent.backend_type.as_deref() == Some("docker");
    if !is_docker {
        bail!(
            "Agent '{}' uses the {} backend. The 'logs' command is only available for Docker-backed agents.",
            agent.name,
            agent.backend_type.as_deref().unwrap_or("tmux")
        );
    }

    let container = agent
        .session_id
        .as_deref()
        .context(format!("Agent '{}' has no container ID. It may have crashed.", agent.name))?;

    if std::process::Command::new("docker")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        bail!("docker is required but not found. Install Docker Desktop or Docker Engine.");
    }

    let mut args = vec!["logs", "--tail", tail];
    if follow {
        args.push("-f");
    }
    args.push(container);

    let status = std::process::Command::new("docker")
        .args(&args)
        .status()
        .context("Failed to exec docker logs")?;

    if !status.success() {
        bail!("docker logs exited with status: {}", status);
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

    let base_url = std::env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
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

// -- Model --

#[allow(clippy::too_many_arguments)]
async fn set_model_cmd(
    client: &OrchestratorClient,
    id: Option<&str>,
    name: Option<&str>,
    model: Option<&str>,
    clear: bool,
    restart: bool,
    json: bool,
) -> Result<()> {
    // Resolve agent ID
    let agent_id = match (id, name) {
        (Some(agent_id), _) => parse_uuid(agent_id)?,
        (_, Some(agent_name)) => resolve_agent_id(client, None, Some(agent_name)).await?,
        (None, None) => bail!("Either an agent ID or --name must be provided."),
    };

    // Resolve model value: --clear sets to None, otherwise use the provided model
    let resolved_model = if clear {
        None
    } else {
        match model {
            Some(m) => Some(m.to_string()),
            None => bail!("Either a model name or --clear must be provided."),
        }
    };

    let agent = client
        .set_model(&agent_id, resolved_model.clone(), restart)
        .await
        .context("Failed to set agent model")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        let model_display = agent.config.model.as_deref().unwrap_or("(default)");
        println!(
            "{}",
            format!("Model updated for agent '{}' ({}): {}", agent.name, agent.id, model_display)
                .green()
                .bold()
        );
        if restart {
            println!("{}", "Agent restarted with new model.".cyan());
        } else {
            println!("{}", "Model will take effect on next agent restart.".yellow());
        }
    }

    Ok(())
}

// -- Usage & context --

async fn usage_cmd(
    client: &OrchestratorClient,
    id: Option<&str>,
    name: Option<&str>,
    json: bool,
) -> Result<()> {
    let agent_id = match (id, name) {
        (Some(agent_id), _) => parse_uuid(agent_id)?,
        (_, Some(agent_name)) => resolve_agent_id(client, None, Some(agent_name)).await?,
        (None, None) => bail!("Either an agent ID or --name must be provided."),
    };

    let stats =
        client.get_agent_usage(&agent_id).await.context("Failed to get agent usage stats")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        display_usage_stats(&stats);
    }

    Ok(())
}

async fn clear_context_cmd(
    client: &OrchestratorClient,
    id: Option<&str>,
    name: Option<&str>,
    json: bool,
) -> Result<()> {
    let agent_id = match (id, name) {
        (Some(agent_id), _) => parse_uuid(agent_id)?,
        (_, Some(agent_name)) => resolve_agent_id(client, None, Some(agent_name)).await?,
        (None, None) => bail!("Either an agent ID or --name must be provided."),
    };

    let response =
        client.clear_context(&agent_id).await.context("Failed to clear agent context")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        display_clear_context_response(&response);
    }

    Ok(())
}

fn display_usage_stats(stats: &AgentUsageStats) {
    println!("{}", "Agent Usage Statistics".blue().bold());
    println!("{}", "=".repeat(60).cyan());
    println!("{}: {}", "Agent ID".bold(), stats.agent_id);
    println!("{}: {}", "Session Count".bold(), stats.session_count);
    println!();

    if let Some(ref session) = stats.current_session {
        println!("{}", "Current Session".green().bold());
        println!("{}", "-".repeat(60).cyan());
        display_session_usage(session);
        println!();
    } else {
        println!("{}", "No active session.".yellow());
        println!();
    }

    println!("{}", "Cumulative (All Sessions)".green().bold());
    println!("{}", "-".repeat(60).cyan());
    display_session_usage(&stats.cumulative);
}

fn display_session_usage(usage: &SessionUsage) {
    println!("  {}: {}", "Input Tokens".bold(), format_tokens(usage.input_tokens));
    println!("  {}: {}", "Output Tokens".bold(), format_tokens(usage.output_tokens));
    println!("  {}: {}", "Cache Read Tokens".bold(), format_tokens(usage.cache_read_input_tokens));
    println!(
        "  {}: {}",
        "Cache Creation Tokens".bold(),
        format_tokens(usage.cache_creation_input_tokens)
    );
    println!("  {}: ${:.4}", "Total Cost".bold(), usage.total_cost_usd);
    println!("  {}: {}", "Turns".bold(), usage.num_turns);
    println!("  {}: {}", "Duration".bold(), format_duration_ms(usage.duration_ms));
    println!("  {}: {}", "API Duration".bold(), format_duration_ms(usage.duration_api_ms));
    println!("  {}: {}", "Results".bold(), usage.result_count);
    println!("  {}: {}", "Started".bold(), usage.started_at);
    if let Some(ended) = usage.ended_at {
        println!("  {}: {}", "Ended".bold(), ended);
    }
}

fn display_clear_context_response(response: &ClearContextResponse) {
    println!("{}", "Context cleared successfully!".green().bold());
    println!("{}", "=".repeat(60).cyan());
    println!("{}: {}", "Agent ID".bold(), response.agent_id);
    println!("{}: {}", "New Session Number".bold(), response.new_session_number);

    if let Some(ref usage) = response.session_usage {
        println!();
        println!("{}", "Session Usage at Clear".yellow().bold());
        println!("{}", "-".repeat(60).cyan());
        display_session_usage(usage);
    }
}

fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn format_duration_ms(ms: u64) -> String {
    if ms >= 60_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m {}s", mins, secs)
    } else if ms >= 1_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}ms", ms)
    }
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
    trigger_type: &TriggerType,
    owner: Option<&str>,
    repo: Option<&str>,
    labels: Option<&str>,
    state: Option<&str>,
    cron_expression: Option<&str>,
    run_at: Option<&str>,
    webhook_secret: Option<&str>,
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

    // Build trigger config based on trigger type with validation.
    let trigger_config = match trigger_type {
        TriggerType::GithubIssues => {
            let owner = owner
                .ok_or_else(|| anyhow::anyhow!("--owner is required for github-issues trigger"))?;
            let repo = repo
                .ok_or_else(|| anyhow::anyhow!("--repo is required for github-issues trigger"))?;
            let labels_vec: Vec<String> = labels
                .map(|l| l.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            TriggerConfig::GithubIssues {
                owner: owner.to_string(),
                repo: repo.to_string(),
                labels: labels_vec,
                state: state.unwrap_or("open").to_string(),
            }
        }
        TriggerType::GithubPullRequests => {
            let owner = owner.ok_or_else(|| {
                anyhow::anyhow!("--owner is required for github-pull-requests trigger")
            })?;
            let repo = repo.ok_or_else(|| {
                anyhow::anyhow!("--repo is required for github-pull-requests trigger")
            })?;
            let labels_vec: Vec<String> = labels
                .map(|l| l.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            TriggerConfig::GithubPullRequests {
                owner: owner.to_string(),
                repo: repo.to_string(),
                labels: labels_vec,
                state: state.unwrap_or("open").to_string(),
            }
        }
        TriggerType::Cron => {
            let expression = cron_expression
                .ok_or_else(|| anyhow::anyhow!("--cron-expression is required for cron trigger"))?;
            TriggerConfig::Cron { expression: expression.to_string() }
        }
        TriggerType::Delay => {
            let run_at_val =
                run_at.ok_or_else(|| anyhow::anyhow!("--run-at is required for delay trigger"))?;
            TriggerConfig::Delay { run_at: run_at_val.to_string() }
        }
        TriggerType::Webhook => {
            TriggerConfig::Webhook { secret: webhook_secret.map(|s| s.to_string()) }
        }
        TriggerType::Manual => TriggerConfig::Manual {},
    };

    let request = CreateWorkflowRequest {
        name: name.to_string(),
        agent_id: resolved_agent_id,
        trigger_config,
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
    if let Some(ref backend) = agent.backend_type {
        let backend_display = match backend.as_str() {
            "docker" => backend.cyan(),
            _ => backend.normal(),
        };
        println!("{}: {}", "Backend".bold(), backend_display);
    }
    if let Some(session) = &agent.session_id {
        println!("{}: {}", "Session".bold(), session);
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

    // Docker-specific details
    if agent.backend_type.as_deref() == Some("docker") {
        if let Some(ref image) = agent.config.docker_image {
            println!("{}: {}", "Docker Image".bold(), image.cyan());
        }
        if let Some(ref limits) = agent.config.resource_limits {
            if let Some(cpu) = limits.cpu_limit {
                println!("{}: {}", "CPU Limit".bold(), cpu);
            }
            if let Some(mem) = limits.memory_limit_mb {
                println!("{}: {} MB", "Memory Limit".bold(), mem);
            }
        }
        if let Some(ref mounts) = agent.config.extra_mounts {
            for mount in mounts {
                let ro = if mount.read_only { ":ro" } else { "" };
                println!("{}: {}:{}{}", "Mount".bold(), mount.host_path, mount.container_path, ro);
            }
        }
        if let Some(ref policy) = agent.config.network_policy {
            println!("{}: {}", "Network Policy".bold(), policy);
        }
    }

    // Additional dirs (always shown)
    let dirs_display = if agent.config.additional_dirs.is_empty() {
        "(none)".dimmed().to_string()
    } else {
        agent.config.additional_dirs.join(", ")
    };
    println!("{}: {}", "Additional Dirs".bold(), dirs_display);

    println!("{}: {}", "Created".bold(), agent.created_at);
}

fn display_workflow(workflow: &WorkflowResponse) {
    println!("{}: {}", "ID".bold(), workflow.id);
    println!("{}: {}", "Name".bold(), workflow.name.bright_white());
    println!("{}: {}", "Agent ID".bold(), workflow.agent_id);
    let status = if workflow.enabled { "enabled".green() } else { "disabled".red() };
    println!("{}: {}", "Status".bold(), status);
    println!("{}: {}s", "Poll Interval".bold(), workflow.poll_interval_secs);
    println!("{}: {}", "Trigger Type".bold(), workflow.trigger_config.trigger_type());
    match &workflow.trigger_config {
        TriggerConfig::GithubIssues { owner, repo, labels, state } => {
            println!("{}: {}/{}", "Repository".bold(), owner, repo);
            if !labels.is_empty() {
                println!("{}: {}", "Labels".bold(), labels.join(", "));
            }
            println!("{}: {}", "State".bold(), state);
        }
        TriggerConfig::GithubPullRequests { owner, repo, labels, state } => {
            println!("{}: {}/{}", "Repository".bold(), owner, repo);
            if !labels.is_empty() {
                println!("{}: {}", "Labels".bold(), labels.join(", "));
            }
            println!("{}: {}", "State".bold(), state);
        }
        TriggerConfig::Cron { expression } => {
            println!("{}: {}", "Expression".bold(), expression);
        }
        TriggerConfig::Delay { run_at } => {
            println!("{}: {}", "Run At".bold(), run_at);
        }
        TriggerConfig::Webhook { secret } => {
            let secret_display = if secret.is_some() { "configured" } else { "none" };
            println!("{}: {}", "Secret".bold(), secret_display);
        }
        TriggerConfig::Manual {} => {}
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
                env: Default::default(),
                auto_clear_threshold: None,
                network_policy: None,
                docker_image: None,
                extra_mounts: None,
                resource_limits: None,
                additional_dirs: vec![],
            },
            session_id: Some("agentd-orch-abc123".to_string()),
            backend_type: Some("tmux".to_string()),
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
            trigger_config: TriggerConfig::GithubIssues {
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

    // -- parse_env_vars unit tests --

    #[test]
    fn test_parse_env_vars_simple() {
        let raw = vec!["ANTHROPIC_API_KEY=sk-ant-test".to_string()];
        let env = parse_env_vars(&raw).unwrap();
        assert_eq!(env.get("ANTHROPIC_API_KEY"), Some(&"sk-ant-test".to_string()));
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn test_parse_env_vars_multiple() {
        let raw = vec![
            "ANTHROPIC_API_KEY=sk-ant-test".to_string(),
            "ANTHROPIC_BASE_URL=https://example.com".to_string(),
            "ANTHROPIC_AUTH_TOKEN=tok-123".to_string(),
        ];
        let env = parse_env_vars(&raw).unwrap();
        assert_eq!(env.len(), 3);
        assert_eq!(env.get("ANTHROPIC_API_KEY"), Some(&"sk-ant-test".to_string()));
        assert_eq!(env.get("ANTHROPIC_BASE_URL"), Some(&"https://example.com".to_string()));
        assert_eq!(env.get("ANTHROPIC_AUTH_TOKEN"), Some(&"tok-123".to_string()));
    }

    #[test]
    fn test_parse_env_vars_value_with_equals() {
        // Value itself contains '=' — only the first '=' is the delimiter
        let raw = vec!["TOKEN=abc=def=ghi".to_string()];
        let env = parse_env_vars(&raw).unwrap();
        assert_eq!(env.get("TOKEN"), Some(&"abc=def=ghi".to_string()));
    }

    #[test]
    fn test_parse_env_vars_empty_value() {
        // KEY= with no value is valid — value is empty string
        let raw = vec!["MY_VAR=".to_string()];
        let env = parse_env_vars(&raw).unwrap();
        assert_eq!(env.get("MY_VAR"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_env_vars_missing_equals_returns_error() {
        let raw = vec!["ANTHROPIC_API_KEY".to_string()]; // no '='
        let result = parse_env_vars(&raw);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("ANTHROPIC_API_KEY"), "error should mention the offending value");
        assert!(msg.contains("KEY=VALUE"), "error should mention the expected format");
    }

    #[test]
    fn test_parse_env_vars_empty_slice() {
        let env = parse_env_vars(&[]).unwrap();
        assert!(env.is_empty());
    }

    #[test]
    fn test_parse_env_vars_url_value() {
        // URLs contain ':' and '/' which must be preserved verbatim
        let raw = vec!["ANTHROPIC_BASE_URL=https://proxy.example.com:8080/v1".to_string()];
        let env = parse_env_vars(&raw).unwrap();
        assert_eq!(
            env.get("ANTHROPIC_BASE_URL"),
            Some(&"https://proxy.example.com:8080/v1".to_string())
        );
    }

    // -- --env flag clap parsing tests --

    #[test]
    fn test_create_agent_env_flag_single() {
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
            "my-agent",
            "--env",
            "ANTHROPIC_API_KEY=sk-ant-test",
        ])
        .expect("Should parse --env flag");

        if let OrchestratorCommand::CreateAgent { env_vars, .. } = cli.command {
            assert_eq!(env_vars, vec!["ANTHROPIC_API_KEY=sk-ant-test"]);
        } else {
            panic!("Expected CreateAgent variant");
        }
    }

    #[test]
    fn test_create_agent_env_flag_multiple() {
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
            "my-agent",
            "--env",
            "ANTHROPIC_API_KEY=sk-ant-test",
            "--env",
            "ANTHROPIC_BASE_URL=https://example.com",
        ])
        .expect("Should parse multiple --env flags");

        if let OrchestratorCommand::CreateAgent { env_vars, .. } = cli.command {
            assert_eq!(env_vars.len(), 2);
            assert!(env_vars.contains(&"ANTHROPIC_API_KEY=sk-ant-test".to_string()));
            assert!(env_vars.contains(&"ANTHROPIC_BASE_URL=https://example.com".to_string()));
        } else {
            panic!("Expected CreateAgent variant");
        }
    }

    #[test]
    fn test_create_agent_env_flag_defaults_to_empty() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from(["test", "create-agent", "--name", "my-agent"])
            .expect("Should parse without --env");

        if let OrchestratorCommand::CreateAgent { env_vars, .. } = cli.command {
            assert!(env_vars.is_empty(), "--env should default to empty Vec");
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

    #[test]
    fn test_set_model_by_id() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from([
            "test",
            "set-model",
            "550e8400-e29b-41d4-a716-446655440000",
            "--model",
            "opus",
            "--restart",
        ])
        .expect("Should parse set-model with ID");

        if let OrchestratorCommand::SetModel { id, name, model, clear, restart } = cli.command {
            assert_eq!(id, Some("550e8400-e29b-41d4-a716-446655440000".to_string()));
            assert_eq!(name, None);
            assert_eq!(model, Some("opus".to_string()));
            assert!(!clear);
            assert!(restart);
        } else {
            panic!("Expected SetModel variant");
        }
    }

    #[test]
    fn test_set_model_by_name() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli =
            Cli::try_parse_from(["test", "set-model", "--name", "worker", "--model", "sonnet"])
                .expect("Should parse set-model with --name");

        if let OrchestratorCommand::SetModel { id, name, model, restart, .. } = cli.command {
            assert_eq!(id, None);
            assert_eq!(name, Some("worker".to_string()));
            assert_eq!(model, Some("sonnet".to_string()));
            assert!(!restart);
        } else {
            panic!("Expected SetModel variant");
        }
    }

    #[test]
    fn test_set_model_clear() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let cli = Cli::try_parse_from([
            "test",
            "set-model",
            "550e8400-e29b-41d4-a716-446655440000",
            "--clear",
        ])
        .expect("Should parse set-model with --clear");

        if let OrchestratorCommand::SetModel { id, model, clear, .. } = cli.command {
            assert_eq!(id, Some("550e8400-e29b-41d4-a716-446655440000".to_string()));
            assert_eq!(model, None);
            assert!(clear);
        } else {
            panic!("Expected SetModel variant");
        }
    }

    #[test]
    fn test_set_model_clear_and_model_conflict() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let result = Cli::try_parse_from([
            "test",
            "set-model",
            "550e8400-e29b-41d4-a716-446655440000",
            "--model",
            "opus",
            "--clear",
        ]);

        assert!(result.is_err(), "--clear and --model should conflict");
    }

    #[test]
    fn test_set_model_id_and_name_conflict() {
        use clap::Parser;

        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: OrchestratorCommand,
        }

        let result = Cli::try_parse_from([
            "test",
            "set-model",
            "550e8400-e29b-41d4-a716-446655440000",
            "--name",
            "worker",
            "--model",
            "opus",
        ]);

        assert!(result.is_err(), "ID and --name should conflict");
    }

    #[test]
    fn test_parse_mount_flags_basic() {
        let mounts = vec!["/host/path:/container/path".to_string()];
        let result = parse_mount_flags(&mounts).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].host_path, "/host/path");
        assert_eq!(result[0].container_path, "/container/path");
        assert!(!result[0].read_only);
    }

    #[test]
    fn test_parse_mount_flags_read_only() {
        let mounts = vec!["/host:/container:ro".to_string()];
        let result = parse_mount_flags(&mounts).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].read_only);
    }

    #[test]
    fn test_parse_mount_flags_read_write() {
        let mounts = vec!["/host:/container:rw".to_string()];
        let result = parse_mount_flags(&mounts).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].read_only);
    }

    #[test]
    fn test_parse_mount_flags_invalid_mode() {
        let mounts = vec!["/host:/container:xyz".to_string()];
        let result = parse_mount_flags(&mounts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid mount mode"));
    }

    #[test]
    fn test_parse_mount_flags_invalid_format() {
        let mounts = vec!["no-colon".to_string()];
        let result = parse_mount_flags(&mounts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid --mount value"));
    }

    #[test]
    fn test_parse_mount_flags_multiple() {
        let mounts = vec!["/a:/b".to_string(), "/c:/d:ro".to_string(), "/e:/f:rw".to_string()];
        let result = parse_mount_flags(&mounts).unwrap();
        assert_eq!(result.len(), 3);
        assert!(!result[0].read_only);
        assert!(result[1].read_only);
        assert!(!result[2].read_only);
    }

    #[test]
    fn test_parse_mount_flags_empty() {
        let mounts: Vec<String> = vec![];
        let result = parse_mount_flags(&mounts).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_display_agent_with_docker_backend() {
        use chrono::Utc;
        let agent = AgentResponse {
            id: Uuid::new_v4(),
            name: "docker-agent".to_string(),
            status: AgentStatus::Running,
            config: AgentConfig {
                working_dir: "/workspace".to_string(),
                user: None,
                shell: "bash".to_string(),
                interactive: false,
                prompt: None,
                worktree: false,
                system_prompt: None,
                tool_policy: Default::default(),
                model: Some("opus".to_string()),
                env: Default::default(),
                auto_clear_threshold: None,
                network_policy: Some(wrap::docker::NetworkPolicy::Internet),
                docker_image: Some("custom:latest".to_string()),
                extra_mounts: Some(vec![orchestrator::types::VolumeMount {
                    host_path: "/host".to_string(),
                    container_path: "/container".to_string(),
                    read_only: true,
                }]),
                resource_limits: Some(orchestrator::types::ResourceLimits {
                    cpu_limit: Some(2.0),
                    memory_limit_mb: Some(4096),
                }),
                additional_dirs: vec![],
            },
            session_id: Some("abc123container".to_string()),
            backend_type: Some("docker".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        // Should not panic and should display Docker-specific details
        display_agent(&agent);
    }

    #[test]
    fn test_display_workflow_all_trigger_types() {
        use chrono::Utc;
        let base = || WorkflowResponse {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            agent_id: Uuid::new_v4(),
            trigger_config: TriggerConfig::Manual {},
            prompt_template: "Do: {{title}}".to_string(),
            poll_interval_secs: 60,
            enabled: true,
            tool_policy: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // All variants should display without panic.
        let mut w = base();
        w.trigger_config = TriggerConfig::GithubPullRequests {
            owner: "org".into(),
            repo: "repo".into(),
            labels: vec!["review".into()],
            state: "open".into(),
        };
        display_workflow(&w);

        w.trigger_config = TriggerConfig::Cron { expression: "0 */6 * * *".into() };
        display_workflow(&w);

        w.trigger_config = TriggerConfig::Delay { run_at: "2026-12-01T00:00:00Z".into() };
        display_workflow(&w);

        w.trigger_config = TriggerConfig::Webhook { secret: Some("s3cret".into()) };
        display_workflow(&w);

        w.trigger_config = TriggerConfig::Webhook { secret: None };
        display_workflow(&w);

        w.trigger_config = TriggerConfig::Manual {};
        display_workflow(&w);
    }

    #[test]
    fn test_create_workflow_trigger_type_github_pr() {
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
            "pr-review",
            "--agent-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--trigger-type",
            "github-pull-requests",
            "--owner",
            "acme",
            "--repo",
            "widgets",
            "--prompt-template",
            "Review: {{title}}",
        ])
        .expect("Should parse with --trigger-type github-pull-requests");

        if let OrchestratorCommand::CreateWorkflow { trigger_type, owner, repo, .. } = cli.command {
            assert!(matches!(trigger_type, TriggerType::GithubPullRequests));
            assert_eq!(owner, Some("acme".to_string()));
            assert_eq!(repo, Some("widgets".to_string()));
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }

    #[test]
    fn test_create_workflow_trigger_type_manual() {
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
            "manual-wf",
            "--agent-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--trigger-type",
            "manual",
            "--prompt-template",
            "Do: {{title}}",
        ])
        .expect("Should parse with --trigger-type manual (no --owner/--repo required)");

        if let OrchestratorCommand::CreateWorkflow { trigger_type, owner, repo, .. } = cli.command {
            assert!(matches!(trigger_type, TriggerType::Manual));
            assert_eq!(owner, None);
            assert_eq!(repo, None);
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }

    #[test]
    fn test_create_workflow_trigger_type_cron() {
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
            "cron-wf",
            "--agent-id",
            "550e8400-e29b-41d4-a716-446655440000",
            "--trigger-type",
            "cron",
            "--cron-expression",
            "0 */6 * * *",
            "--prompt-template",
            "Run scheduled task",
        ])
        .expect("Should parse with --trigger-type cron");

        if let OrchestratorCommand::CreateWorkflow { trigger_type, cron_expression, .. } =
            cli.command
        {
            assert!(matches!(trigger_type, TriggerType::Cron));
            assert_eq!(cron_expression, Some("0 */6 * * *".to_string()));
        } else {
            panic!("Expected CreateWorkflow variant");
        }
    }

    #[test]
    fn test_trigger_config_serde_roundtrip() {
        // New trigger types serialize/deserialize correctly.
        let configs = vec![
            TriggerConfig::Cron { expression: "0 */6 * * *".into() },
            TriggerConfig::Delay { run_at: "2026-12-01T00:00:00Z".into() },
            TriggerConfig::Webhook { secret: Some("s3cret".into()) },
            TriggerConfig::Webhook { secret: None },
            TriggerConfig::Manual {},
        ];
        for config in configs {
            let json = serde_json::to_string(&config).unwrap();
            let roundtripped: TriggerConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(config.trigger_type(), roundtripped.trigger_type());
        }
    }

    #[test]
    fn test_trigger_config_backward_compat_source_config() {
        // Existing JSON payloads with "source_config" key still deserialize.
        let json = r#"{
            "name": "test",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "source_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets"
            },
            "prompt_template": "Fix: {{title}}"
        }"#;
        let req: CreateWorkflowRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.trigger_config.trigger_type(), "github_issues");
    }

    #[test]
    fn test_trigger_config_new_key_accepted() {
        // New JSON payloads with "trigger_config" key also deserialize.
        let json = r#"{
            "name": "test",
            "agent_id": "550e8400-e29b-41d4-a716-446655440000",
            "trigger_config": {
                "type": "github_issues",
                "owner": "acme",
                "repo": "widgets"
            },
            "prompt_template": "Fix: {{title}}"
        }"#;
        let req: CreateWorkflowRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.trigger_config.trigger_type(), "github_issues");
    }

    #[test]
    fn test_trigger_config_is_poll_based() {
        assert!(TriggerConfig::GithubIssues {
            owner: "a".into(),
            repo: "b".into(),
            labels: vec![],
            state: "open".into(),
        }
        .is_poll_based());
        assert!(TriggerConfig::GithubPullRequests {
            owner: "a".into(),
            repo: "b".into(),
            labels: vec![],
            state: "open".into(),
        }
        .is_poll_based());
        assert!(!TriggerConfig::Cron { expression: "* * * * *".into() }.is_poll_based());
        assert!(!TriggerConfig::Delay { run_at: "2026-01-01T00:00:00Z".into() }.is_poll_based());
        assert!(!TriggerConfig::Webhook { secret: None }.is_poll_based());
        assert!(!TriggerConfig::Manual {}.is_poll_based());
    }
}
