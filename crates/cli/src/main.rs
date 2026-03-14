//! Command-line interface for the agentd service ecosystem.
//!
//! The `agent` CLI provides a unified interface for interacting with multiple services:
//! - **Notification Service** (port 17004): Manage notifications from various sources
//! - **Ask Service** (port 17001): Trigger checks and answer questions
//! - **Orchestrator Service** (port 17006): Manage agents and workflows
//! - **Wrap Service** (port 17005): Launch agents in tmux sessions
//! - **Hook Daemon**: Git and system hooks integration (coming soon)
//! - **Monitor Daemon**: System monitoring and alerts (coming soon)
//!
//! # Usage
//!
//! ## Notification Commands
//!
//! Create a notification:
//! ```bash
//! agent notify create \
//!   --title "Build Failed" \
//!   --message "Tests failed on main branch" \
//!   --priority high \
//!   --requires-response
//! ```
//!
//! List all notifications:
//! ```bash
//! agent notify list
//! ```
//!
//! List only actionable notifications:
//! ```bash
//! agent notify list --actionable
//! ```
//!
//! Get a specific notification:
//! ```bash
//! agent notify get <notification-id>
//! ```
//!
//! Respond to a notification:
//! ```bash
//! agent notify respond <notification-id> "This is my response"
//! ```
//!
//! Delete a notification:
//! ```bash
//! agent notify delete <notification-id>
//! ```
//!
//! ## Ask Service Commands
//!
//! Trigger checks in the ask service:
//! ```bash
//! agent ask trigger
//! ```
//!
//! Answer a question:
//! ```bash
//! agent ask answer <question-id> "This is my answer"
//! ```
//!
//! # Service URLs
//!
//! The CLI connects to services running on localhost (default dev ports):
//! - Notification service: `http://localhost:7004` (override with `AGENTD_NOTIFY_SERVICE_URL`)
//! - Ask service: `http://localhost:7001` (override with `AGENTD_ASK_SERVICE_URL`)
//! - Wrap service: `http://localhost:7005` (override with `AGENTD_WRAP_SERVICE_URL`)
//! - Orchestrator service: `http://localhost:7006` (override with `AGENTD_ORCHESTRATOR_SERVICE_URL`)
//!
//! # Architecture
//!
//! The CLI uses a REST API client to communicate with backend services. All commands
//! are async and use Tokio runtime for efficient I/O operations.

pub mod client;
mod commands;
pub mod types;

use anyhow::Result;
use ask::client::AskClient;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use colored::*;
use commands::{AskCommand, MemoryCommand, NotifyCommand, OrchestratorCommand, WrapCommand};
use memory::client::MemoryClient;
use notify::client::NotifyClient;
use orchestrator::client::OrchestratorClient;
use std::env;
use wrap::client::WrapClient;

/// Main CLI structure parsed by clap.
///
/// This is the entry point for all agent commands. The CLI uses a subcommand
/// pattern where each major service has its own subcommand namespace.
#[derive(Parser)]
#[command(name = "agent")]
#[command(author, version, about = "CLI for interacting with agentd services", long_about = None)]
struct Cli {
    /// Output raw JSON responses instead of formatted text
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

/// Top-level commands for the agent CLI.
///
/// Each variant represents a major service or daemon in the agentd ecosystem.
/// Commands are dispatched to their respective handlers which communicate with
/// backend services via REST APIs.
#[derive(Subcommand)]
enum Commands {
    /// Interact with the notification service
    ///
    /// Manage notifications from various sources including agent hooks, ask service,
    /// monitor service, and system notifications. The notification service runs on
    /// port 7004 by default.
    Notify {
        #[command(subcommand)]
        command: NotifyCommand,
    },
    /// Interact with the ask service
    ///
    /// Trigger periodic checks and answer questions from the ask service. The ask
    /// service runs on port 7001 by default and can create notifications when checks
    /// require user attention.
    Ask {
        #[command(subcommand)]
        command: AskCommand,
    },
    /// Interact with the wrap service
    ///
    /// Launch and manage agents in tmux sessions. The wrap service runs on
    /// port 7005 by default and handles agent lifecycle management including
    /// launching agents with proper configuration and monitoring their health.
    Wrap {
        #[command(subcommand)]
        command: WrapCommand,
    },
    /// Interact with the orchestrator service
    ///
    /// Manage AI agents and autonomous workflows. The orchestrator service
    /// runs on port 7006 by default and handles agent lifecycle management,
    /// workflow scheduling, and task dispatch.
    Orchestrator {
        #[command(subcommand)]
        command: OrchestratorCommand,
    },
    /// Generate shell completion scripts.
    ///
    /// Outputs a completion script for the specified shell to stdout.
    /// Redirect the output to the appropriate file for your shell.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Bash
    /// agent completions bash > ~/.local/share/bash-completion/completions/agent
    ///
    /// # Zsh
    /// agent completions zsh > ~/.zfunc/_agent
    ///
    /// # Fish
    /// agent completions fish > ~/.config/fish/completions/agent.fish
    ///
    /// # PowerShell
    /// agent completions powershell > _agent.ps1
    /// ```
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Apply agent and workflow templates from a YAML file or .agentd/ directory.
    ///
    /// Creates agents first, waits for them to start, then creates workflows
    /// that reference those agents by name.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent apply .agentd/                                    # full project
    /// agent apply .agentd/workflows/issue-worker.yml          # single workflow
    /// agent apply --dry-run .agentd/                          # validate only
    /// agent apply --wait-timeout 120 .agentd/                 # custom timeout
    /// ```
    Apply {
        /// Path to a YAML template file or .agentd/ directory
        path: std::path::PathBuf,
        /// Validate only, don't create anything
        #[arg(long)]
        dry_run: bool,
        /// Seconds to wait for agents to reach running status (default: 60)
        #[arg(long, default_value = "60")]
        wait_timeout: u64,
    },

    /// Teardown resources defined in a .agentd/ directory.
    ///
    /// Deletes workflows first, then agents (reverse of apply order).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent teardown .agentd/
    /// agent teardown --dry-run .agentd/
    /// ```
    Teardown {
        /// Path to the .agentd/ directory
        path: std::path::PathBuf,
        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
    },

    /// Check the health of all agentd services.
    ///
    /// Checks all services concurrently and displays a summary table.
    Status,

    /// Start the hook daemon
    ///
    /// The hook daemon monitors shell and git hook events, recording them and
    /// creating notifications when user intervention may be required.
    /// Default port: 17002 (dev) / 7002 (production).
    Hook,
    /// Start the monitor daemon
    ///
    /// The monitor daemon watches system metrics and creates notifications for
    /// alerts and anomalies.
    Monitor,
    /// Interact with the memory service
    ///
    /// Store, retrieve, and semantically search agent memory records. The memory
    /// service runs on port 7008 by default and uses LanceDB for vector storage
    /// with SQLite for metadata.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

/// Main entry point for the agent CLI.
///
/// Parses command-line arguments using clap and dispatches to the appropriate
/// command handler. Uses Tokio async runtime for all I/O operations.
///
/// # Service Connections
///
/// - Notify commands connect to `http://localhost:7004`
/// - Ask commands connect to `http://localhost:7001`
/// - Wrap commands connect to `http://localhost:7005`
/// - Orchestrator commands connect to `http://localhost:7006`
///
/// # Error Handling
///
/// All errors are propagated up and handled by the anyhow error type, which
/// provides rich context and backtraces in debug mode.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Notify { command } => {
            // Use AGENTD_NOTIFY_SERVICE_URL env var, default to production port
            let url = env::var("AGENTD_NOTIFY_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7004".to_string());
            let client = NotifyClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Ask { command } => {
            // Use AGENTD_ASK_SERVICE_URL env var, default to production port
            let url = env::var("AGENTD_ASK_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7001".to_string());
            let client = AskClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Wrap { command } => {
            // Use AGENTD_WRAP_SERVICE_URL env var, default to production port
            let url = env::var("AGENTD_WRAP_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7005".to_string());
            let client = WrapClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Orchestrator { command } => {
            // Use AGENTD_ORCHESTRATOR_SERVICE_URL env var, default to production port
            let url = env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7006".to_string());
            let client = OrchestratorClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Apply { path, dry_run, wait_timeout } => {
            let url = env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7006".to_string());
            let client = OrchestratorClient::new(url);
            if path.is_dir() {
                commands::apply::apply_directory(&client, &path, dry_run, wait_timeout, cli.json)
                    .await?;
            } else {
                match commands::apply::detect_template_kind(&path)? {
                    commands::apply::TemplateKind::Agent => {
                        commands::apply::apply_agent_file(&client, &path, dry_run, cli.json)
                            .await?;
                    }
                    commands::apply::TemplateKind::Workflow => {
                        commands::apply::apply_workflow_file(&client, &path, dry_run, cli.json)
                            .await?;
                    }
                }
            }
        }
        Commands::Teardown { path, dry_run } => {
            let url = env::var("AGENTD_ORCHESTRATOR_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7006".to_string());
            let client = OrchestratorClient::new(url);
            commands::apply::teardown_directory(&client, &path, dry_run, cli.json).await?;
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "agent", &mut std::io::stdout());
        }
        Commands::Status => {
            check_all_services(cli.json).await?;
        }
        Commands::Hook => {
            hook::run(hook::config::HookConfig::from_env()).await?;
        }
        Commands::Monitor => {
            monitor::run(monitor::config::MonitorConfig::from_env()).await?;
        }
        Commands::Memory { command } => {
            // Use AGENTD_MEMORY_SERVICE_URL env var, default to production port
            let url = env::var("AGENTD_MEMORY_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7008".to_string());
            let client = MemoryClient::new(url);
            command.execute(&client, cli.json).await?;
        }
    }

    Ok(())
}

struct ServiceDef {
    name: &'static str,
    env_var: &'static str,
    default_url: &'static str,
}

const SERVICES: &[ServiceDef] = &[
    ServiceDef {
        name: "orchestrator",
        env_var: "AGENTD_ORCHESTRATOR_SERVICE_URL",
        default_url: "http://localhost:7006",
    },
    ServiceDef {
        name: "notify",
        env_var: "AGENTD_NOTIFY_SERVICE_URL",
        default_url: "http://localhost:7004",
    },
    ServiceDef {
        name: "ask",
        env_var: "AGENTD_ASK_SERVICE_URL",
        default_url: "http://localhost:7001",
    },
    ServiceDef {
        name: "wrap",
        env_var: "AGENTD_WRAP_SERVICE_URL",
        default_url: "http://localhost:7005",
    },
    ServiceDef {
        name: "hook",
        env_var: "AGENTD_HOOK_SERVICE_URL",
        default_url: "http://localhost:7002",
    },
    ServiceDef {
        name: "monitor",
        env_var: "AGENTD_MONITOR_SERVICE_URL",
        default_url: "http://localhost:7003",
    },
    ServiceDef {
        name: "memory",
        env_var: "AGENTD_MEMORY_SERVICE_URL",
        default_url: "http://localhost:7008",
    },
];

#[derive(serde::Serialize)]
struct ServiceStatus {
    name: String,
    url: String,
    healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn check_all_services(json: bool) -> Result<()> {
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(3)).build()?;

    let checks: Vec<(&str, String)> = SERVICES
        .iter()
        .map(|svc| {
            let url = env::var(svc.env_var).unwrap_or_else(|_| svc.default_url.to_string());
            (svc.name, url)
        })
        .collect();

    let mut handles = Vec::new();
    for (name, url) in &checks {
        let client = http.clone();
        let health_url = format!("{}/health", url);
        let name = name.to_string();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::json!({}));
                    let detail = body
                        .get("agents_active")
                        .and_then(|v| v.as_u64())
                        .map(|n| format!("{n} agents active"));
                    ServiceStatus { name, url, healthy: true, detail, error: None }
                }
                Ok(resp) => ServiceStatus {
                    name,
                    url,
                    healthy: false,
                    detail: None,
                    error: Some(format!("HTTP {}", resp.status())),
                },
                Err(e) => {
                    let msg = if e.is_connect() {
                        "connection refused".to_string()
                    } else if e.is_timeout() {
                        "timeout".to_string()
                    } else {
                        e.to_string()
                    };
                    ServiceStatus { name, url, healthy: false, detail: None, error: Some(msg) }
                }
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await?);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    println!("{}", "agentd Service Status".blue().bold());
    println!("{}", "=".repeat(60).cyan());

    let healthy_count = results.iter().filter(|r| r.healthy).count();
    let total = results.len();

    for status in &results {
        let indicator = if status.healthy { "✅" } else { "❌" };
        let name_padded = format!("{:<14}", status.name);
        let url_display = format!("({})", status.url).bright_black();

        if status.healthy {
            let detail = status.detail.as_deref().unwrap_or("");
            let detail_display = if detail.is_empty() {
                "ok".green().to_string()
            } else {
                format!("{}  ({})", "ok".green(), detail.cyan())
            };
            println!("  {} {} {}  {}", indicator, name_padded.bold(), url_display, detail_display);
        } else {
            let err = status.error.as_deref().unwrap_or("unknown error");
            println!("  {} {} {}  {}", indicator, name_padded.bold(), url_display, err.red());
        }
    }

    println!();
    println!("{}/{} services healthy", healthy_count.to_string().green().bold(), total);

    Ok(())
}
