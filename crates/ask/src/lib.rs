//! Ask Service — Agent-to-human question/answer system.
//!
//! The `agentd-ask` service provides a REST API for AI agents to ask questions
//! to the human user. Questions persist until answered and can trigger workflow
//! callbacks in the orchestrator when answered.
//!
//! # Features
//!
//! - **Agent questions**: Agents create questions via `POST /questions`
//! - **Human answers**: Humans answer via `POST /questions/{id}/answer`
//! - **Lifecycle tracking**: Pending → Answered / Dismissed / Expired
//! - **Workflow integration**: Orchestrator callback on answer for reactive workflows
//!
//! # Architecture
//!
//! - [`api`] - HTTP endpoints and routing
//! - [`client`] - HTTP client for making requests to the ask service
//! - [`state`] - Thread-safe application state
//! - [`storage`] - Persistent question storage (SQLite via SeaORM)
//! - [`types`] - Request/response types and data structures
//! - [`error`] - Error types and HTTP response conversions

pub mod api;
pub mod client;
pub mod entity;
pub mod error;
pub mod migration;
pub mod state;
pub mod storage;
pub mod types;
