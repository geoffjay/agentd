//! Wrap service command implementations.
//!
//! This module implements all subcommands for managing agent launches via the wrap service.
//! The wrap service is responsible for launching agent CLIs in tmux sessions and monitoring
//! their lifecycle.
//!
//! # Available Commands
//!
//! - **launch**: Launch an agent in a tmux session with specified configuration
//!
//! # Examples
//!
//! ## Launch a Claude Code agent
//!
//! ```bash
//! agentd wrap launch \
//!   --session-name my-session \
//!   --path /path/to/project \
//!   --agent claude-code \
//!   --provider anthropic \
//!   --model claude-sonnet-4.5
//! ```
//!
//! ## Launch with custom layout
//!
//! ```bash
//! agentd wrap launch \
//!   --session-name my-session \
//!   --path /path/to/project \
//!   --agent opencode \
//!   --provider openai \
//!   --model gpt-4 \
//!   --layout '{"type":"vertical"}'
//! ```
//!
//! # Output Formatting
//!
//! - **launch**: Displays session information and launch status
//!
//! All output uses colored terminal formatting for better readability.

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use wrap::client::WrapClient;
use wrap::types::*;

/// Wrap service management subcommands.
///
/// Each variant corresponds to a specific operation on the wrap service.
/// All commands communicate with the wrap service REST API on port 7002.
#[derive(Subcommand)]
pub enum WrapCommand {
    /// Check the health of the wrap service.
    Health,

    /// List all active tmux sessions.
    ///
    /// Shows all tmux sessions managed by the wrap service.
    List,

    /// Kill a tmux session by name.
    ///
    /// Terminates the specified tmux session and any processes running in it.
    Kill {
        /// Session name to kill
        name: String,
    },

    /// Launch an agent in a tmux session.
    ///
    /// Creates a new tmux session and starts the specified agent with the
    /// given configuration. The agent will run in the background in the
    /// tmux session.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Launch Claude Code
    /// agentd wrap launch my-project \
    ///   --path /home/user/projects/my-project \
    ///   --agent claude-code \
    ///   --provider anthropic \
    ///   --model claude-sonnet-4.5
    ///
    /// # Launch with custom tmux layout
    /// agentd wrap launch my-project \
    ///   --agent opencode \
    ///   --provider openai \
    ///   --model gpt-4 \
    ///   --layout-json '{"type":"vertical","panes":2}'
    /// ```
    Launch {
        /// Session name (required)
        session_name: String,

        /// Working directory path for the agent (defaults to current directory)
        #[arg(long, default_value = ".")]
        path: String,

        /// Agent type (claude-code, crush, opencode, gemini, etc.)
        #[arg(long)]
        agent: Option<String>,

        /// Model provider (anthropic, openai, ollama, etc.)
        #[arg(long)]
        provider: Option<String>,

        /// Model name (claude-sonnet-4.5, gpt-4, etc.)
        #[arg(long)]
        model: Option<String>,

        /// Optional tmux layout configuration as JSON string
        #[arg(long)]
        layout_json: Option<String>,
    },
}

impl WrapCommand {
    /// Execute the wrap command by dispatching to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `client` - The wrap service client
    /// * `json` - If true, output raw JSON instead of formatted text
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the command fails.
    pub async fn execute(&self, client: &WrapClient, json: bool) -> Result<()> {
        match self {
            WrapCommand::Health => wrap_health(client, json).await,
            WrapCommand::List => list_sessions(client, json).await,
            WrapCommand::Kill { name } => kill_session(client, name, json).await,
            WrapCommand::Launch { session_name, path, agent, provider, model, layout_json } => {
                launch_agent(
                    client,
                    session_name,
                    path,
                    agent.as_deref(),
                    provider.as_deref(),
                    model.as_deref(),
                    layout_json.as_deref(),
                    json,
                )
                .await
            }
        }
    }
}

async fn wrap_health(client: &WrapClient, json: bool) -> Result<()> {
    client.health().await.context("Failed to reach wrap service. Is it running?")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok"}))?);
    } else {
        println!("{} {}", "wrap:".bold(), "ok".green().bold());
    }

    Ok(())
}

/// List all active tmux sessions.
async fn list_sessions(client: &WrapClient, json: bool) -> Result<()> {
    let response = client.list_sessions().await.context("Failed to list sessions")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.sessions.is_empty() {
        println!("{}", "No active sessions.".yellow());
    } else {
        println!("{}", "Active Sessions:".blue().bold());
        println!("{}", "=".repeat(60).cyan());
        for session in &response.sessions {
            println!(
                "  {}: {}",
                session.name.bright_white(),
                if session.active { "active".green() } else { "inactive".red() }
            );
        }
        println!("{}", "=".repeat(60).cyan());
        println!("Total: {} session(s)", response.count);
    }

    Ok(())
}

/// Kill a tmux session by name.
async fn kill_session(client: &WrapClient, name: &str, json: bool) -> Result<()> {
    let response =
        client.kill_session(name).await.context(format!("Failed to kill session '{}'", name))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.success {
        println!("{}", format!("Session '{}' terminated.", name).green().bold());
    } else {
        println!("{}", format!("Failed to kill session: {}", response.message).red());
    }

    Ok(())
}

/// Launch an agent in a tmux session via POST request to the API.
///
/// Creates a `LaunchRequest` with the provided configuration and sends it
/// to the wrap service. The service will create a tmux session and start
/// the agent with the specified parameters.
///
/// # Arguments
///
/// * `client` - Wrap service client
/// * `session_name` - Session name for tmux
/// * `path` - Working directory path for the agent (can be relative)
/// * `agent` - Optional agent type (claude-code, opencode, gemini, etc.)
/// * `provider` - Optional model provider (anthropic, openai, ollama, etc.)
/// * `model` - Optional model name (claude-sonnet-4.5, gpt-4, etc.)
/// * `layout_json` - Optional tmux layout configuration as JSON string
/// * `json` - Whether to output raw JSON
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the launch response.
///
/// # Errors
///
/// Returns an error if:
/// - The network request fails
/// - The API returns an error
/// - The agent fails to launch
#[allow(clippy::too_many_arguments)]
async fn launch_agent(
    client: &WrapClient,
    session_name: &str,
    path: &str,
    agent: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
    layout_json: Option<&str>,
    json: bool,
) -> Result<()> {
    // Expand tilde and resolve the path to an absolute path
    let expanded_path = if path.starts_with("~/") {
        // Expand ~ to home directory
        if let Ok(home) = std::env::var("HOME") {
            path.replacen("~", &home, 1)
        } else {
            path.to_string()
        }
    } else if path == "~" {
        // Handle bare ~ (though unlikely for project paths)
        std::env::var("HOME").unwrap_or_else(|_| path.to_string())
    } else {
        path.to_string()
    };

    let absolute_path = std::path::PathBuf::from(&expanded_path)
        .canonicalize()
        .context(format!("Failed to resolve project path: {}", expanded_path))?;
    let project_path = absolute_path.to_str().context("Invalid UTF-8 in project path")?.to_string();

    // Parse layout JSON if provided
    let layout_obj = if let Some(layout_str) = layout_json {
        Some(serde_json::from_str::<TmuxLayout>(layout_str).context("Invalid layout JSON")?)
    } else {
        None
    };

    // Use defaults for optional fields
    let agent_type = agent.unwrap_or("claude-code").to_string();
    let model_provider = provider.unwrap_or("anthropic").to_string();
    let model_name = model.unwrap_or("claude-sonnet-4.5").to_string();

    let request = LaunchRequest {
        project_name: session_name.to_string(),
        project_path,
        agent_type,
        model_provider,
        model_name,
        layout: layout_obj,
    };

    let response = client.launch(&request).await.context("Failed to launch agent")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        if response.success {
            println!("{}", "Agent launched successfully!".green().bold());
        } else {
            println!("{}", "Agent launch failed!".red().bold());
        }
        println!();
        display_launch_response(&response);
    }

    Ok(())
}

/// Display a launch response with formatted, colored output.
///
/// Shows all response fields with visual separators and color-coded
/// success/failure indicators.
///
/// # Arguments
///
/// * `response` - The launch response to display
fn display_launch_response(response: &LaunchResponse) {
    println!("{}", "=".repeat(80).cyan());
    println!(
        "{}: {}",
        "Status".bold(),
        if response.success { "Success".green() } else { "Failed".red() }
    );

    if let Some(ref session_name) = response.session_name {
        println!("{}: {}", "Session Name".bold(), session_name.bright_white());
    }

    println!("{}: {}", "Message".bold(), response.message);

    if let Some(ref error) = response.error {
        println!("{}: {}", "Error".bold().red(), error.bright_red());
    }

    println!("{}", "=".repeat(80).cyan());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_launch_response_success() {
        let response = LaunchResponse {
            success: true,
            session_name: Some("test-session".to_string()),
            message: "Agent launched successfully".to_string(),
            error: None,
        };

        // This should not panic
        display_launch_response(&response);
    }

    #[test]
    fn test_display_launch_response_failure() {
        let response = LaunchResponse {
            success: false,
            session_name: None,
            message: "Failed to start tmux session".to_string(),
            error: Some("Failed to start tmux session".to_string()),
        };

        // This should not panic
        display_launch_response(&response);
    }
}
