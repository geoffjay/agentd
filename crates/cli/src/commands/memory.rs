//! Memory service command implementations.
//!
//! This module provides CLI subcommands for interacting with the agentd-memory
//! service. Commands for storing, retrieving, and searching memory records via
//! semantic search will be added in subsequent issues.
//!
//! # Available Commands
//!
//! - **health**: Check the health of the memory service
//!
//! # Examples
//!
//! ## Check memory service health
//!
//! ```bash
//! agent memory health
//! ```

use anyhow::Result;
use clap::Subcommand;
use colored::*;

/// Subcommands for the memory service.
#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Check the health of the memory service
    Health,
}

impl MemoryCommand {
    /// Execute the memory subcommand.
    pub async fn execute(&self, base_url: &str, _json: bool) -> Result<()> {
        match self {
            MemoryCommand::Health => {
                let url = format!("{}/health", base_url);
                let client = reqwest::Client::new();
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let body: serde_json::Value = resp.json().await?;
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    }
                    Ok(resp) => {
                        println!("{} HTTP {}", "error:".red().bold(), resp.status());
                    }
                    Err(e) => {
                        println!("{} {}", "error:".red().bold(), e);
                    }
                }
            }
        }
        Ok(())
    }
}
