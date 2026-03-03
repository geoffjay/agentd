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
/// All commands communicate with the ask service REST API on port 7001.
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
    /// * `json` - If true, output raw JSON instead of formatted text
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the command fails.
    pub async fn execute(&self, client: &AskClient, json: bool) -> Result<()> {
        match self {
            AskCommand::Trigger => trigger_checks(client, json).await,
            AskCommand::Answer { question_id, answer } => {
                answer_question(client, question_id, answer, json).await
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
async fn trigger_checks(client: &AskClient, json: bool) -> Result<()> {
    let response = client
        .trigger_checks()
        .await
        .context("Failed to trigger checks. Is the ask service running?")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    println!("{}", "Triggering ask service checks...".cyan());
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
async fn answer_question(
    client: &AskClient,
    question_id: &str,
    answer: &str,
    json: bool,
) -> Result<()> {
    let uuid = Uuid::parse_str(question_id).context("Invalid question UUID format")?;

    let request = AnswerRequest { question_id: uuid, answer: answer.to_string() };

    let response = client
        .answer_question(&request)
        .await
        .context("Failed to submit answer. Is the ask service running?")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", format!("Submitting answer to question {uuid}...").cyan());
        println!();
        println!("{}", "✓ Answer submitted successfully!".green().bold());
        println!();
        println!("{}: {}", "Message".bold(), response.message);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ask::types::{AnswerResponse, TriggerResponse};

    #[test]
    fn test_trigger_response_json_deserialization() {
        let json = r#"{
            "checks_run": ["tmux_sessions"],
            "notifications_sent": [],
            "results": {
                "tmux_sessions": {
                    "running": true,
                    "session_count": 2
                }
            }
        }"#;

        let response: TriggerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.checks_run, vec!["tmux_sessions"]);
        assert!(response.notifications_sent.is_empty());
        assert!(response.results.tmux_sessions.running);
        assert_eq!(response.results.tmux_sessions.session_count, 2);
    }

    #[test]
    fn test_answer_request_serialization() {
        let request = AnswerRequest {
            question_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            answer: "yes".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("550e8400"));
        assert!(json.contains("yes"));
    }

    #[test]
    fn test_answer_response_deserialization() {
        let json = r#"{
            "success": true,
            "message": "Answer recorded",
            "question_id": "550e8400-e29b-41d4-a716-446655440000"
        }"#;

        let response: AnswerResponse = serde_json::from_str(json).unwrap();
        assert!(response.success);
        assert_eq!(response.message, "Answer recorded");
    }
}
