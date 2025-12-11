//! Rust client for BAML (Basically a Made-up Language) server.
//!
//! This crate provides a type-safe Rust client for interacting with a BAML server
//! running via `baml serve`. The BAML server provides AI-powered functions for
//! intelligent automation throughout the agentd project.
//!
//! # Features
//!
//! - **Notification Intelligence**: Auto-categorization, digests, and relevance filtering
//! - **Smart Questions**: Context-aware question generation for the ask service
//! - **Log Analysis**: Automated error detection and root cause analysis
//! - **CLI Intelligence**: Natural language command parsing and help
//! - **Hook Analysis**: Smart filtering of shell events for notifications
//!
//! # Quick Start
//!
//! ```no_run
//! use baml::{BamlClient, BamlClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client (assumes BAML server running on localhost:2024)
//!     let client = BamlClient::default();
//!
//!     // Categorize a notification
//!     let result = client.categorize_notification(
//!         "Database Error",
//!         "Connection to PostgreSQL failed",
//!         "production monitoring"
//!     ).await?;
//!
//!     println!("Category: {}", result.category);
//!     println!("Priority: {}", result.priority);
//!     println!("Reasoning: {}", result.reasoning);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Configuration
//!
//! By default, the client connects to `http://localhost:2024`. You can customize this:
//!
//! ```no_run
//! use baml::{BamlClient, BamlClientConfig};
//!
//! let config = BamlClientConfig::new("http://baml-server:8080")
//!     .with_timeout(60)        // 60 second timeout
//!     .with_max_retries(3);    // retry up to 3 times
//!
//! let client = BamlClient::new(config);
//! ```
//!
//! # Starting the BAML Server
//!
//! Before using this client, you must start the BAML server:
//!
//! ```bash
//! # From the project root
//! baml serve
//! ```
//!
//! The server will load all BAML function definitions from `baml_src/` and expose
//! them via a REST API.
//!
//! # Error Handling
//!
//! All client methods return `Result<T, BamlError>`. Common errors include:
//!
//! - `ServerUnreachable`: BAML server is not running or not accessible
//! - `FunctionNotFound`: BAML function doesn't exist (typo or version mismatch)
//! - `Timeout`: Request took longer than configured timeout
//! - `ServerError`: BAML server returned an error (check server logs)
//!
//! # Integration with Services
//!
//! ## agentd-notify
//!
//! ```no_run
//! # use baml::BamlClient;
//! # async fn example(client: BamlClient, title: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
//! // Auto-categorize incoming notifications
//! let category = client.categorize_notification(
//!     title,
//!     message,
//!     "notification-service"
//! ).await?;
//!
//! // Use the AI-suggested priority
//! let priority = category.priority;
//! # Ok(())
//! # }
//! ```
//!
//! ## agentd-ask
//!
//! ```no_run
//! # use baml::BamlClient;
//! # async fn example(client: BamlClient) -> Result<(), Box<dyn std::error::Error>> {
//! // Generate contextual questions
//! let question = client.generate_system_question(
//!     "tmux_sessions",
//!     "0 sessions running",
//!     "User in terminal",
//!     "Last session ended 2 hours ago"
//! ).await?;
//!
//! println!("Ask user: {}", question.question_text);
//! # Ok(())
//! # }
//! ```
//!
//! ## agentd-monitor
//!
//! ```no_run
//! # use baml::BamlClient;
//! # async fn example(client: BamlClient, logs: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
//! // Analyze service logs
//! let analysis = client.analyze_logs(
//!     "agentd-notify",
//!     &logs,
//!     "last 5 minutes"
//! ).await?;
//!
//! if analysis.has_errors {
//!     println!("Issues detected: {}", analysis.error_summary);
//! }
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod error;
pub mod types;

pub use client::{BamlClient, BamlClientConfig};
pub use error::{BamlError, Result};
pub use types::*;
