//! agentd-orchestrator — Agent lifecycle management and workflow orchestration.
//!
//! This crate provides the orchestrator service that manages AI agent processes,
//! implements the Claude Code SDK WebSocket protocol, schedules autonomous
//! workflows, and enforces tool policies.
//!
//! # Modules
//!
//! - [`api`] — REST API handlers and router (agents, workflows, approvals, policies)
//! - [`approvals`] — In-memory registry for human-in-the-loop tool approval requests
//! - [`client`] — Typed HTTP client (`OrchestratorClient`) for consuming the REST API
//! - [`manager`] — Agent lifecycle: spawn, reconcile, terminate tmux sessions
//! - [`scheduler`] — Autonomous workflow scheduling with GitHub issue polling
//! - [`storage`] — SQLite persistence for agent and workflow state
//! - [`types`] — Domain types: `Agent`, `AgentConfig`, `ToolPolicy`, etc.
//! - [`websocket`] — WebSocket SDK server and real-time monitoring streams
//!
//! # Configuration
//!
//! - **Default port:** 17006 (dev) / 7006 (production)
//! - **Database:** `~/Library/Application Support/agentd-orchestrator/orchestrator.db`
//! - **Environment:** `PORT`, `RUST_LOG`, `LOG_FORMAT`

pub mod api;
pub mod approvals;
pub mod client;
pub mod manager;
pub mod scheduler;
pub mod storage;
pub mod types;
pub mod websocket;
