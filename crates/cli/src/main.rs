//! Command-line interface for the agentd service ecosystem.
//!
//! The `agentd` CLI provides a unified interface for interacting with multiple services:
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
//! agentd notify create \
//!   --title "Build Failed" \
//!   --message "Tests failed on main branch" \
//!   --priority high \
//!   --requires-response
//! ```
//!
//! List all notifications:
//! ```bash
//! agentd notify list
//! ```
//!
//! List only actionable notifications:
//! ```bash
//! agentd notify list --actionable
//! ```
//!
//! Get a specific notification:
//! ```bash
//! agentd notify get <notification-id>
//! ```
//!
//! Respond to a notification:
//! ```bash
//! agentd notify respond <notification-id> "This is my response"
//! ```
//!
//! Delete a notification:
//! ```bash
//! agentd notify delete <notification-id>
//! ```
//!
//! ## Ask Service Commands
//!
//! Trigger checks in the ask service:
//! ```bash
//! agentd ask trigger
//! ```
//!
//! Answer a question:
//! ```bash
//! agentd ask answer <question-id> "This is my answer"
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
use clap::{Parser, Subcommand};
use client::ApiClient;
use commands::{AskCommand, NotifyCommand};

/// Main CLI structure parsed by clap.
///
/// This is the entry point for all agentd commands. The CLI uses a subcommand
/// pattern where each major service has its own subcommand namespace.
#[derive(Parser)]
#[command(name = "agentd")]
#[command(author, version, about = "CLI for interacting with agentd services", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Top-level commands for the agentd CLI.
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

/// Main entry point for the agentd CLI.
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
    // If called as "agent" with no arguments, launch the GUI
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        // No arguments provided - launch GUI
        launch_gui()?;
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Notify { command } => {
            let client = ApiClient::new("http://localhost:3000".to_string());
            command.execute(&client).await?;
        }
        Commands::Ask { command } => {
            let client = ApiClient::new("http://localhost:3001".to_string());
            command.execute(&client).await?;
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

/// Launch the GUI application.
///
/// On macOS, this uses the `open` command to launch Agent.app.
/// The GUI binary is located at /Applications/Agent.app/Contents/MacOS/agent.
fn launch_gui() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        println!("Launching Agent GUI...");

        Command::new("open")
            .arg("-a")
            .arg("Agent")
            .spawn()
            .context("Failed to launch Agent.app. Is it installed?")?;

        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("GUI is only supported on macOS");
    }
}
