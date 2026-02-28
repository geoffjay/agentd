//! Command-line interface for the agentd service ecosystem.
//!
//! The `agent` CLI provides a unified interface for interacting with multiple services:
//! - **Notification Service** (port 3000): Manage notifications from various sources
//! - **Ask Service** (port 3001): Trigger checks and answer questions
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
//! The CLI connects to services running on localhost:
//! - Notification service: `http://localhost:3000`
//! - Ask service: `http://localhost:3001`
//!
//! # Architecture
//!
//! The CLI uses a REST API client to communicate with backend services. All commands
//! are async and use Tokio runtime for efficient I/O operations.

pub mod client;
mod commands;
pub mod types;

use anyhow::{Context, Result};
use ask::client::AskClient;
use clap::{Parser, Subcommand};
use commands::{AskCommand, NotifyCommand, WrapCommand};
use notify::client::NotifyClient;
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
    /// port 3000 by default.
    Notify {
        #[command(subcommand)]
        command: NotifyCommand,
    },
    /// Interact with the ask service
    ///
    /// Trigger periodic checks and answer questions from the ask service. The ask
    /// service runs on port 3001 by default and can create notifications when checks
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
    /// Start the hook daemon
    ///
    /// The hook daemon monitors git hooks and other system hooks, creating
    /// notifications when user intervention is required. (Not yet implemented)
    Hook,
    /// Start the monitor daemon
    ///
    /// The monitor daemon watches system metrics and creates notifications for
    /// alerts and anomalies. (Not yet implemented)
    Monitor,
}

/// Main entry point for the agent CLI.
///
/// Parses command-line arguments using clap and dispatches to the appropriate
/// command handler. Uses Tokio async runtime for all I/O operations.
///
/// # Service Connections
///
/// - Notify commands connect to `http://localhost:3000`
/// - Ask commands connect to `http://localhost:3001`
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
            // Use NOTIFY_SERVICE_URL env var, default to production port
            let url = env::var("NOTIFY_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7004".to_string());
            let client = NotifyClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Ask { command } => {
            // Use ASK_SERVICE_URL env var, default to production port
            let url =
                env::var("ASK_SERVICE_URL").unwrap_or_else(|_| "http://localhost:7001".to_string());
            let client = AskClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Wrap { command } => {
            // Use WRAP_SERVICE_URL env var, default to production port
            let url = env::var("WRAP_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:7005".to_string());
            let client = WrapClient::new(url);
            command.execute(&client, cli.json).await?;
        }
        Commands::Hook => {
            println!("Starting hook daemon...");
            // TODO: Start hook daemon
        }
        Commands::Monitor => {
            println!("Starting monitor daemon...");
            // TODO: Start monitor daemon
        }
    }

    Ok(())
}
