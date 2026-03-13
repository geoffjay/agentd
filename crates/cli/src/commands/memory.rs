//! Memory service command implementations.
//!
//! Provides CLI subcommands for interacting with the agentd-memory service.
//! Additional subcommands (store, search, get, delete, visibility) will be
//! added in subsequent issues once the storage layer is implemented.
//!
//! # Available Commands
//!
//! - **health**: Check the health of the memory service
//!
//! # Examples
//!
//! ```bash
//! agent memory health
//! ```

use anyhow::Result;
use clap::Subcommand;
use colored::*;

/// Subcommands for the memory service.
///
/// Stub implementation — full CRUD and search commands will be added once
/// the storage and embedding backends are wired up (see issue #302+).
#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Check the health of the memory service.
    Health,
}

impl MemoryCommand {
    /// Execute the memory subcommand against the service at `base_url`.
    pub async fn execute(&self, base_url: &str, json: bool) -> Result<()> {
        match self {
            MemoryCommand::Health => {
                let url = format!("{}/health", base_url);
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()?;

                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let body: serde_json::Value = resp.json().await?;
                        if json {
                            println!("{}", serde_json::to_string_pretty(&body)?);
                        } else {
                            let status = body["status"].as_str().unwrap_or("unknown");
                            let version = body["version"].as_str().unwrap_or("unknown");
                            println!(
                                "{} agentd-memory {} (v{})",
                                "✅".green(),
                                status.green(),
                                version
                            );
                        }
                    }
                    Ok(resp) => {
                        let code = resp.status();
                        if json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(
                                    &serde_json::json!({"error": format!("HTTP {}", code)})
                                )?
                            );
                        } else {
                            println!("{} HTTP {}", "❌".red(), code);
                        }
                    }
                    Err(e) => {
                        let msg = if e.is_connect() {
                            "connection refused".to_string()
                        } else if e.is_timeout() {
                            "timeout".to_string()
                        } else {
                            e.to_string()
                        };
                        if json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(
                                    &serde_json::json!({"error": msg})
                                )?
                            );
                        } else {
                            println!("{} {}", "❌".red(), msg.red());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
