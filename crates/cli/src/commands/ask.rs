//! Ask service command implementations.
//!
//! This module implements commands for interacting with the ask service, which
//! performs periodic checks and creates notifications when conditions require
//! user attention.
//!
//! # Available Commands
//!
//! - **trigger**: Manually trigger all registered checks in the ask service
//! - **answer**: Submit an answer to a specific question
//!
//! # Examples
//!
//! ## Trigger checks
//!
//! ```bash
//! agentd ask trigger
//! ```
//!
//! This runs all configured checks and may create notifications if any checks
//! require user attention.
//!
//! ## Answer a question
//!
//! ```bash
//! agentd ask answer 550e8400-e29b-41d4-a716-446655440000 "Yes, proceed"
//! ```
//!
//! # Output Formatting
//!
//! - **trigger**: Shows summary of checks performed and notifications created
//! - **answer**: Confirms successful submission
//!
//! All output uses colored terminal formatting for better readability.

use anyhow::{Context, Result};
use ask::client::AskClient;
use ask::types::AnswerRequest;
use clap::Subcommand;
use colored::*;
use uuid::Uuid;

/// Ask service subcommands.
///
/// Each variant corresponds to a specific operation on the ask service.
/// All commands communicate with the ask service REST API on port 3001.
#[derive(Subcommand)]
pub enum AskCommand {
    /// Trigger all checks in the ask service.
    ///
    /// This manually runs all registered checks (e.g., checking if there are
    /// unread notifications, pending reviews, etc.). If any checks identify
    /// conditions requiring attention, notifications will be created.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask trigger
    /// ```
    Trigger,

    /// Answer a specific question from the ask service.
    ///
    /// Questions are identified by UUID and typically correspond to
    /// notifications that require user input.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask answer 550e8400-e29b-41d4-a716-446655440000 "Yes, approved"
    /// ```
    Answer {
        /// UUID of the question to answer
        question_id: String,

        /// Answer text (can be multiple words)
        answer: String,
    },
}

impl AskCommand {
    /// Execute the ask command by dispatching to the appropriate handler.
    ///
    /// # Arguments
    ///
    /// * `client` - The ask service client
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the command fails.
    pub async fn execute(&self, client: &AskClient) -> Result<()> {
        match self {
            AskCommand::Trigger => trigger_checks(client).await,
            AskCommand::Answer { question_id, answer } => {
                answer_question(client, question_id, answer).await
            }
        }
    }
}

/// Trigger all checks in the ask service.
///
/// Sends a POST request to `/trigger` which runs all registered checks.
/// Displays a summary of checks performed and notifications created.
///
/// # Arguments
///
/// * `client` - Ask service client
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying the summary.
///
/// # Errors
///
/// Returns an error if the network request fails or the ask service is not running.
async fn trigger_checks(client: &AskClient) -> Result<()> {
    println!("{}", "Triggering ask service checks...".cyan());

    let response = client.trigger_checks().await.context("Failed to trigger checks. Is the ask service running?")?;

    println!();
    println!("{}", "✓ Checks completed successfully!".green().bold());
    println!();
    println!("{}: {}", "Checks Run".bold(), response.checks_run.len().to_string().cyan());
    println!(
        "{}: {}",
        "Notifications Sent".bold(),
        response.notifications_sent.len().to_string().yellow()
    );

    if !response.notifications_sent.is_empty() {
        println!();
        println!(
            "{}",
            "Tip: Use 'agent notify list --actionable' to see new notifications.".bright_black()
        );
    }

    Ok(())
}

/// Submit an answer to a question.
///
/// Sends a POST request to `/answer` with the question ID and answer text.
/// Displays confirmation when the answer is successfully submitted.
///
/// # Arguments
///
/// * `client` - Ask service client
/// * `question_id` - UUID of the question as a string
/// * `answer` - User's answer text
///
/// # Returns
///
/// Returns `Ok(())` on success after displaying confirmation.
///
/// # Errors
///
/// Returns an error if:
/// - The question ID is not a valid UUID
/// - The network request fails
/// - The ask service is not running
/// - The question is not found
async fn answer_question(client: &AskClient, question_id: &str, answer: &str) -> Result<()> {
    let uuid = Uuid::parse_str(question_id).context("Invalid question UUID format")?;

    println!("{}", format!("Submitting answer to question {uuid}...").cyan());

    let request = AnswerRequest { question_id: uuid, answer: answer.to_string() };

    let response = client
        .answer_question(&request)
        .await
        .context("Failed to submit answer. Is the ask service running?")?;

    println!();
    println!("{}", "✓ Answer submitted successfully!".green().bold());
    println!();
    println!("{}: {}", "Message".bold(), response.message);

    Ok(())
}
