//! Hook Service - Shell and git hook integration daemon.
//!
//! The `agentd-hook` service receives shell and git hook events and creates
//! notifications in the notify service when user intervention is required.
//!
//! # Features
//!
//! - **Hook Events**: Receive and record shell/git hook events via REST
//! - **Notification Integration**: Forward important events to the notify service
//! - **REST API**: Simple HTTP endpoints for health and event ingestion
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint.
//!
//! ```bash
//! curl http://localhost:17002/health
//! ```
//!
//! ## POST /events
//!
//! Receive a hook event (shell command completion, git hook, etc.).
//!
//! ```bash
//! curl -X POST http://localhost:17002/events \
//!   -H "Content-Type: application/json" \
//!   -d '{"kind":"shell","command":"cargo build","exit_code":0,"duration_ms":1200}'
//! ```
//!
//! # Environment Variables
//!
//! - `PORT` - Port to listen on (default: 17002 dev, 7002 production)
//! - `RUST_LOG` - Logging level (default: info)

pub mod api;
pub mod error;
pub mod types;
