//! Ask Service - Interactive notification service with tmux integration.
//!
//! The `agentd-ask` service provides a REST API for triggering environment checks
//! and sending interactive notifications that require user responses. It integrates
//! with the notification service to create questions and track user answers.
//!
//! # Features
//!
//! - **Environment Checks**: Automatically checks for running tmux sessions
//! - **Interactive Notifications**: Creates notifications that require user responses
//! - **Cooldown Management**: Prevents notification spam with configurable cooldowns
//! - **Question Tracking**: Maintains state of pending, answered, and expired questions
//! - **REST API**: Simple HTTP endpoints for triggering checks and providing answers
//!
//! # Architecture
//!
//! The service consists of several key modules:
//!
//! - [`api`] - HTTP endpoints and routing
//! - [`client`] - HTTP client for making requests to the ask service
//! - [`state`] - Thread-safe application state management
//! - [`tmux_check`] - Tmux session detection and monitoring
//! - [`notification_client`] - HTTP client for notification service
//! - [`types`] - Request/response types and data structures
//! - [`error`] - Error types and HTTP response conversions
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint that returns service status and configuration.
//!
//! ```bash
//! curl http://localhost:3001/health
//! ```
//!
//! ## POST /trigger
//!
//! Triggers environment checks and sends notifications if conditions are met.
//! Currently checks for running tmux sessions and asks the user if they want
//! to start one if none are running.
//!
//! ```bash
//! curl -X POST http://localhost:3001/trigger
//! ```
//!
//! ## POST /answer
//!
//! Submits an answer to a pending question.
//!
//! ```bash
//! curl -X POST http://localhost:3001/answer \
//!   -H "Content-Type: application/json" \
//!   -d '{"question_id": "uuid-here", "answer": "yes"}'
//! ```
//!
//! # Environment Variables
//!
//! - `ASK_PORT` - Port to listen on (default: 3001)
//! - `NOTIFY_SERVICE_URL` - URL of notification service (default: http://localhost:3000)
//! - `RUST_LOG` - Logging level (default: info)
//!
//! # Examples
//!
//! ## Using the service programmatically
//!
//! ```no_run
//! use ask::{api::ApiState, state::AppState, notification_client::NotificationClient};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize application state
//!     let app_state = AppState::new();
//!
//!     // Create notification client
//!     let notification_client = NotificationClient::new(
//!         "http://localhost:3000".to_string()
//!     );
//!
//!     // Create API state
//!     let api_state = ApiState {
//!         app_state,
//!         notification_client,
//!         notification_service_url: "http://localhost:3000".to_string(),
//!     };
//!
//!     // Create router and serve...
//! }
//! ```
//!
//! # Integration with Notification Service
//!
//! This service depends on the `agentd-notify` service for creating and managing
//! notifications. Questions created here are stored as notifications with
//! `requires_response: true`, and answers are sent back to update the notification
//! status.

pub mod api;
pub mod client;
pub mod error;
pub mod notification_client;
pub mod state;
pub mod tmux_check;
pub mod types;
