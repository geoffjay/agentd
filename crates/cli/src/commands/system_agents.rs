//! System-agent CLI command implementations.
//!
//! Provides the `agent system-agents` subcommand group for interacting with
//! built-in system agents.  System agents are distinct from user-created
//! agents: they are spawned automatically by the orchestrator at startup,
//! are always present while the service is running, and cannot be deleted
//! via the user-facing API.
//!
//! # Available Subcommands
//!
//! - **list** — list all built-in system agents
//! - **get** — show full details for a specific system agent
//! - **message** — send a prompt to a running system agent
//! - **status** — compact status summary of all system agents
//!
//! # Examples
//!
//! ```bash
//! agent system-agents list
//! agent system-agents list --json
//! agent system-agents get agentd-system
//! agent system-agents message agentd-system "What services are running?"
//! agent system-agents status
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use uuid::Uuid;

use orchestrator::client::OrchestratorClient;
use orchestrator::types::{AgentResponse, AgentStatus, SendMessageRequest, ToolPolicy};

/// Subcommand group for interacting with built-in system agents.
///
/// System agents are created programmatically by the orchestrator at startup
/// and are always available while the service is running.  Use `agent list`
/// for user-created agents instead.
#[derive(Subcommand)]
pub enum SystemAgentsCommand {
    /// List all built-in system agents.
    ///
    /// Fetches from `GET /system-agents` which returns only agents marked as
    /// built-in.  These are distinct from user-created agents and are managed
    /// automatically by the orchestrator.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent system-agents list
    /// agent system-agents list --json
    /// ```
    List,

    /// Show full details for a specific system agent.
    ///
    /// Accepts either the agent's UUID or its name (e.g., `agentd-system`).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent system-agents get agentd-system
    /// agent system-agents get 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Get {
        /// Agent UUID or name
        id: String,
    },

    /// Send a prompt to a running system agent.
    ///
    /// Accepts either the agent's UUID or its name.  The agent must be in the
    /// `running` state to accept messages.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent system-agents message agentd-system "What services are running?"
    /// agent system-agents message agentd-system "Explain the agent lifecycle"
    /// ```
    Message {
        /// Agent UUID or name
        id: String,
        /// The prompt to send to the agent
        message: String,
    },

    /// Show a compact status summary of all system agents.
    ///
    /// Displays name, status, model, and activity state in a compact table.
    /// Useful for a quick health check of the built-in agent layer.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent system-agents status
    /// ```
    Status,
}

impl SystemAgentsCommand {
    /// Dispatch to the appropriate handler.
    pub async fn execute(&self, client: &OrchestratorClient, json: bool) -> Result<()> {
        match self {
            SystemAgentsCommand::List => list_system_agents(client, json).await,
            SystemAgentsCommand::Get { id } => get_system_agent(client, id, json).await,
            SystemAgentsCommand::Message { id, message } => {
                message_system_agent(client, id, message, json).await
            }
            SystemAgentsCommand::Status => system_agents_status(client, json).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `agent system-agents list` — list all built-in system agents.
async fn list_system_agents(client: &OrchestratorClient, json: bool) -> Result<()> {
    let agents = client.list_system_agents().await.context("Failed to list system agents")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agents)?);
        return Ok(());
    }

    if agents.is_empty() {
        println!("{}", "No system agents found.".yellow());
        println!(
            "{}",
            "The orchestrator may still be starting up. Try again in a moment.".dimmed()
        );
        return Ok(());
    }

    println!("{}", "System Agents:".blue().bold());
    println!("{}", "=".repeat(80).cyan());
    for agent in &agents {
        display_system_agent(agent);
        println!("{}", "-".repeat(80).cyan());
    }
    println!("Total: {} system agent(s)", agents.len());

    Ok(())
}

/// `agent system-agents get <id>` — show full details for a system agent.
async fn get_system_agent(client: &OrchestratorClient, id: &str, json: bool) -> Result<()> {
    let agent = resolve_system_agent(client, id).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
        return Ok(());
    }

    display_system_agent(&agent);
    Ok(())
}

/// `agent system-agents message <id> <text>` — send a prompt to a system agent.
async fn message_system_agent(
    client: &OrchestratorClient,
    id: &str,
    message: &str,
    json: bool,
) -> Result<()> {
    let agent = resolve_system_agent(client, id).await?;

    if agent.status != AgentStatus::Running {
        anyhow::bail!(
            "System agent '{}' is not running (status: {}). \
             Wait for the orchestrator to bootstrap it.",
            agent.name,
            agent.status
        );
    }

    let request = SendMessageRequest { content: message.to_string() };
    let response =
        client.send_message(&agent.id, &request).await.context("Failed to send message")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", "Message sent.".green());
        println!("{}: {}", "Agent".bold(), agent.name.bright_white());
        println!("{}: {}", "Status".bold(), response.status);
    }

    Ok(())
}

/// `agent system-agents status` — compact status summary.
async fn system_agents_status(client: &OrchestratorClient, json: bool) -> Result<()> {
    let agents = client.list_system_agents().await.context("Failed to fetch system agents")?;

    if json {
        // Emit a compact summary array.
        let summaries: Vec<_> = agents
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "status": a.status,
                    "model": a.config.model,
                    "activity": a.activity,
                    "built_in": a.built_in,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&summaries)?);
        return Ok(());
    }

    if agents.is_empty() {
        println!("{}", "No system agents found.".yellow());
        return Ok(());
    }

    println!("{}", "System Agent Status:".blue().bold());
    println!("{}", "=".repeat(60).cyan());

    let name_w = 24usize;
    let status_w = 10usize;
    let model_w = 20usize;

    println!(
        "{:<name_w$} {:<status_w$} {:<model_w$} {}",
        "Name".bold(),
        "Status".bold(),
        "Model".bold(),
        "Activity".bold(),
    );
    println!("{}", "-".repeat(60).dimmed());

    for agent in &agents {
        let status_str = agent.status.to_string();
        let colored_status: colored::ColoredString = match agent.status {
            AgentStatus::Running => status_str.green(),
            AgentStatus::Pending => status_str.yellow(),
            AgentStatus::Stopped => status_str.red(),
            AgentStatus::Failed => status_str.bright_red(),
        };
        let model = agent.config.model.as_deref().unwrap_or("default");
        let activity = format!("{:?}", agent.activity).to_lowercase();

        println!(
            "{:<name_w$} {:<status_w$} {:<model_w$} {}",
            agent.name.bright_white(),
            colored_status,
            model,
            activity,
        );
    }

    println!("{}", "-".repeat(60).dimmed());
    let running = agents.iter().filter(|a| a.status == AgentStatus::Running).count();
    println!("Total: {} system agent(s), {} running", agents.len(), running);

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a system agent by UUID or name.
///
/// Fetches the full system-agent list and looks up by ID prefix or exact name.
/// Returns an error if no match is found.
async fn resolve_system_agent(client: &OrchestratorClient, id: &str) -> Result<AgentResponse> {
    // Try UUID parse first.
    if let Ok(uuid) = Uuid::parse_str(id) {
        let all = client.list_system_agents().await.context("Failed to list system agents")?;
        return all
            .into_iter()
            .find(|a| a.id == uuid)
            .ok_or_else(|| anyhow::anyhow!("System agent '{}' not found", id));
    }

    // Fall back to name lookup.
    let all = client.list_system_agents().await.context("Failed to list system agents")?;
    all.into_iter()
        .find(|a| a.name == id)
        .ok_or_else(|| anyhow::anyhow!("System agent '{}' not found", id))
}

/// Display a single system agent in human-readable format.
fn display_system_agent(agent: &AgentResponse) {
    println!("{}: {}", "ID".bold(), agent.id);
    println!("{}: {} {}", "Name".bold(), agent.name.bright_white(), "[system]".cyan().dimmed());
    let status_str = agent.status.to_string();
    let colored_status = match agent.status {
        AgentStatus::Running => status_str.green(),
        AgentStatus::Pending => status_str.yellow(),
        AgentStatus::Stopped => status_str.red(),
        AgentStatus::Failed => status_str.bright_red(),
    };
    println!("{}: {}", "Status".bold(), colored_status);
    if let Some(ref model) = agent.config.model {
        println!("{}: {}", "Model".bold(), model.cyan());
    }
    if let Some(ref session) = agent.session_id {
        println!("{}: {}", "Session".bold(), session);
    }
    println!("{}: {}", "Working Dir".bold(), agent.config.working_dir);
    if !agent.config.rooms.is_empty() {
        println!("{}: {}", "Rooms".bold(), agent.config.rooms.join(", ").cyan());
    }
    let policy_str = match &agent.config.tool_policy {
        ToolPolicy::AllowAll { .. } => "allow_all".green().to_string(),
        ToolPolicy::DenyAll { .. } => "deny_all".red().to_string(),
        ToolPolicy::AllowList { tools, .. } => {
            format!("allow_list ({} tools)", tools.len())
        }
        ToolPolicy::DenyList { tools, .. } => {
            format!("deny_list ({} tools)", tools.len())
        }
        ToolPolicy::RequireApproval { .. } => "require_approval".bright_yellow().to_string(),
    };
    println!("{}: {}", "Tool Policy".bold(), policy_str);
    println!("{}: {}", "Built-in".bold(), "yes".cyan());
    println!("{}: {}", "Created".bold(), agent.created_at);
}
