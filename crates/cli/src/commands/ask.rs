//! Ask service command implementations (stub — full Q&A commands in issue #925).
//!
//! The old trigger/check commands have been removed. New Q&A commands
//! (create, answer, dismiss, list, get) will be added in issue #925.

use anyhow::Result;
use ask::client::AskClient;
use clap::Subcommand;
use colored::*;

/// Ask service subcommands.
#[derive(Subcommand)]
pub enum AskCommand {
    /// Check the health of the ask service.
    Health,
}

impl AskCommand {
    pub async fn execute(&self, client: &AskClient, _json: bool) -> Result<()> {
        match self {
            AskCommand::Health => {
                let response = client.health().await?;
                println!("{} {}", "Status:".bold(), response.status.green());
                println!("{} {}", "Service:".bold(), response.service);
                println!("{} {}", "Version:".bold(), response.version);
            }
        }
        Ok(())
    }
}
