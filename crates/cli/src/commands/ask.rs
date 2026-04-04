//! Ask service command implementations.
//!
//! This module implements all subcommands for managing agent-to-human Q&A via the REST API.
//! Commands include creating questions, answering, dismissing, listing, and getting questions.
//!
//! # Available Commands
//!
//! - **health**: Check service health
//! - **create**: Create a new question (as if an agent is asking)
//! - **answer**: Provide a human answer to a pending question
//! - **dismiss**: Dismiss a question without answering
//! - **list**: List questions with optional filters
//! - **get**: Retrieve a specific question by UUID
//!
//! # Examples
//!
//! ```bash
//! # Ask a question
//! agentd ask create --agent-id dietician --question "What did you eat yesterday?"
//!
//! # List pending questions
//! agentd ask list --status Pending
//!
//! # Answer a question
//! agentd ask answer <uuid> "I had salad for lunch"
//!
//! # Dismiss a question
//! agentd ask dismiss <uuid>
//! ```

use anyhow::{Context, Result};
use ask::client::AskClient;
use ask::types::{CreateQuestionRequest, ListQuestionsQuery, QuestionPriority};
use clap::Subcommand;
use colored::*;
use uuid::Uuid;

/// Ask service subcommands.
#[derive(Subcommand)]
pub enum AskCommand {
    /// Check the health of the ask service.
    Health,

    /// Create a new question (simulating an agent asking).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask create \
    ///   --agent-id dietician \
    ///   --question "What did you eat yesterday?" \
    ///   --category health \
    ///   --priority normal
    /// ```
    Create {
        /// Agent ID that is asking the question.
        #[arg(short, long)]
        agent_id: String,

        /// The question text.
        #[arg(short, long)]
        question: String,

        /// Optional category for filtering (e.g. "health", "deployment").
        #[arg(short, long)]
        category: Option<String>,

        /// Additional context for the human.
        #[arg(long)]
        context: Option<String>,

        /// Priority level: low, normal, high, or urgent (default: normal).
        #[arg(short, long, default_value = "normal")]
        priority: String,

        /// Time-to-live in seconds (optional).
        #[arg(short, long)]
        expires_in: Option<u64>,
    },

    /// Answer a pending question.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask answer 550e8400-e29b-41d4-a716-446655440000 "I had salad for lunch"
    /// ```
    Answer {
        /// Question UUID to answer.
        id: String,

        /// The answer text.
        answer: String,
    },

    /// Dismiss a pending question without answering.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask dismiss 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Dismiss {
        /// Question UUID to dismiss.
        id: String,
    },

    /// List questions with optional filters.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # List all pending questions
    /// agentd ask list --status Pending
    ///
    /// # List questions from a specific agent
    /// agentd ask list --agent-id dietician
    ///
    /// # List by category with limit
    /// agentd ask list --category health --limit 10
    /// ```
    List {
        /// Filter by status: Pending, Answered, Dismissed, or Expired.
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by agent ID.
        #[arg(short, long)]
        agent_id: Option<String>,

        /// Filter by category.
        #[arg(short, long)]
        category: Option<String>,

        /// Maximum number of results (default: 50).
        #[arg(short, long)]
        limit: Option<u64>,

        /// Offset for pagination (default: 0).
        #[arg(short, long)]
        offset: Option<u64>,
    },

    /// Get detailed information about a specific question.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agentd ask get 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Get {
        /// Question UUID to retrieve.
        id: String,
    },
}

impl AskCommand {
    /// Execute the ask command.
    pub async fn execute(&self, client: &AskClient, json: bool) -> Result<()> {
        match self {
            AskCommand::Health => ask_health(client, json).await,
            AskCommand::Create { agent_id, question, category, context, priority, expires_in } => {
                create_question(
                    client,
                    agent_id,
                    question,
                    category.as_deref(),
                    context.as_deref(),
                    priority,
                    *expires_in,
                    json,
                )
                .await
            }
            AskCommand::Answer { id, answer } => answer_question(client, id, answer, json).await,
            AskCommand::Dismiss { id } => dismiss_question(client, id, json).await,
            AskCommand::List { status, agent_id, category, limit, offset } => {
                list_questions(
                    client,
                    status.as_deref(),
                    agent_id.as_deref(),
                    category.as_deref(),
                    *limit,
                    *offset,
                    json,
                )
                .await
            }
            AskCommand::Get { id } => get_question(client, id, json).await,
        }
    }
}

async fn ask_health(client: &AskClient, json: bool) -> Result<()> {
    let response = client.health().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{} {}", "Status:".bold(), response.status.green());
        println!("{} {}", "Service:".bold(), response.service);
        println!("{} {}", "Version:".bold(), response.version);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_question(
    client: &AskClient,
    agent_id: &str,
    question: &str,
    category: Option<&str>,
    context: Option<&str>,
    priority: &str,
    expires_in: Option<u64>,
    json: bool,
) -> Result<()> {
    let priority: QuestionPriority = priority.parse().context("Invalid priority level")?;

    let req = CreateQuestionRequest {
        agent_id: agent_id.to_string(),
        workflow_id: None,
        dispatch_id: None,
        category: category.map(str::to_string),
        question: question.to_string(),
        context: context.map(str::to_string),
        priority: Some(priority),
        expires_in_seconds: expires_in,
    };

    let q = client.create_question(&req).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&q)?);
    } else {
        println!("{}", "─".repeat(60));
        println!("{} {}", "ID:".bold(), q.id);
        println!("{} {}", "Agent:".bold(), q.agent_id);
        if let Some(cat) = &q.category {
            println!("{} {}", "Category:".bold(), cat);
        }
        println!("{} {}", "Question:".bold(), q.question);
        if let Some(ctx) = &q.context {
            println!("{} {}", "Context:".bold(), ctx);
        }
        println!("{} {}", "Priority:".bold(), format_priority(q.priority.as_str()));
        println!("{} {}", "Status:".bold(), format_status(q.status.as_str()));
        println!("{} {}", "Asked at:".bold(), q.asked_at.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("{}", "─".repeat(60));
    }
    Ok(())
}

async fn answer_question(client: &AskClient, id: &str, answer: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let q = client.answer_question(uuid, answer).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&q)?);
    } else {
        println!("{}", "─".repeat(60));
        println!("{} {}", "ID:".bold(), q.id);
        println!("{} {}", "Status:".bold(), format_status(q.status.as_str()));
        println!("{} {}", "Answer:".bold(), answer);
        if let Some(at) = q.answered_at {
            println!("{} {}", "Answered at:".bold(), at.format("%Y-%m-%d %H:%M:%S UTC"));
        }
        println!("{}", "─".repeat(60));
    }
    Ok(())
}

async fn dismiss_question(client: &AskClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let q = client.dismiss_question(uuid).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&q)?);
    } else {
        println!("{} {} {}", "Question".bold(), q.id.to_string().dimmed(), "dismissed.".bold());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn list_questions(
    client: &AskClient,
    status: Option<&str>,
    agent_id: Option<&str>,
    category: Option<&str>,
    limit: Option<u64>,
    offset: Option<u64>,
    json: bool,
) -> Result<()> {
    let filters = ListQuestionsQuery {
        status: status.map(str::to_string),
        agent_id: agent_id.map(str::to_string),
        category: category.map(str::to_string),
        limit,
        offset,
    };

    let result = client.list_questions(&filters).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    println!("{} {} question(s)", "Total:".bold(), result.total.to_string().cyan());

    if result.questions.is_empty() {
        println!("{}", "No questions found.".dimmed());
        return Ok(());
    }

    println!("{}", "─".repeat(80));
    for q in &result.questions {
        println!(
            "{} {} | {} {} | {} {} | {} {}",
            "ID:".bold(),
            &q.id.to_string()[..8].dimmed(),
            "Agent:".bold(),
            q.agent_id,
            "Priority:".bold(),
            format_priority(q.priority.as_str()),
            "Status:".bold(),
            format_status(q.status.as_str()),
        );
        println!("  {}", q.question.italic());
        if let Some(cat) = &q.category {
            println!("  {} {}", "Category:".dimmed(), cat.dimmed());
        }
        println!("{}", "─".repeat(80));
    }
    Ok(())
}

async fn get_question(client: &AskClient, id: &str, json: bool) -> Result<()> {
    let uuid = parse_uuid(id)?;
    let q = client.get_question(uuid).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&q)?);
        return Ok(());
    }

    println!("{}", "─".repeat(60));
    println!("{} {}", "ID:".bold(), q.id);
    println!("{} {}", "Agent:".bold(), q.agent_id);
    if let Some(wf) = q.workflow_id {
        println!("{} {}", "Workflow:".bold(), wf);
    }
    if let Some(dp) = q.dispatch_id {
        println!("{} {}", "Dispatch:".bold(), dp);
    }
    if let Some(cat) = &q.category {
        println!("{} {}", "Category:".bold(), cat);
    }
    println!("{} {}", "Question:".bold(), q.question);
    if let Some(ctx) = &q.context {
        println!("{} {}", "Context:".bold(), ctx);
    }
    println!("{} {}", "Priority:".bold(), format_priority(q.priority.as_str()));
    println!("{} {}", "Status:".bold(), format_status(q.status.as_str()));
    println!("{} {}", "Asked at:".bold(), q.asked_at.format("%Y-%m-%d %H:%M:%S UTC"));
    if let Some(exp) = q.expires_at {
        println!("{} {}", "Expires at:".bold(), exp.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    if let Some(ans) = &q.answer {
        println!("{} {}", "Answer:".bold(), ans.green());
    }
    if let Some(at) = q.answered_at {
        println!("{} {}", "Answered at:".bold(), at.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    println!("{}", "─".repeat(60));

    Ok(())
}

/// Parse a UUID string, giving a friendly error message.
fn parse_uuid(s: &str) -> Result<Uuid> {
    Uuid::parse_str(s).with_context(|| format!("Invalid UUID: '{s}'"))
}

/// Colorize priority strings for terminal output.
fn format_priority(priority: &str) -> ColoredString {
    match priority {
        "urgent" => priority.red().bold(),
        "high" => priority.yellow().bold(),
        "normal" => priority.cyan(),
        "low" => priority.dimmed(),
        other => other.normal(),
    }
}

/// Colorize status strings for terminal output.
fn format_status(status: &str) -> ColoredString {
    match status {
        "Pending" => status.yellow(),
        "Answered" => status.green(),
        "Dismissed" => status.dimmed(),
        "Expired" => status.red(),
        other => other.normal(),
    }
}
