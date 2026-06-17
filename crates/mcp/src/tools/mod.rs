//! MCP tool implementations for agentd services.
//!
//! Tools are grouped by the agentd service they interact with.
//!
//! # Tool Modules
//!
//! - `agents`             — list/inspect agents
//! - `approvals`          — tool approval requests
//! - `communicate`        — rooms, participants, messages
//! - `creation`           — create agents and create/manage workflows
//! - `diagnostic`         — cross-service diagnostics (system, agent, workflow)
//! - `health`             — service health probes, system metrics, prometheus
//! - `knowledge`          — browse, read, and author per-project markdown documents
//! - `lifecycle`          — restart/terminate agents, send messages, update policy/model
//! - `memory`             — search and list stored memories
//! - `metrics`            — curated Prometheus queries via the monitor service
//! - `notifications`      — list/create/dismiss notifications
//! - `orchestrator_debug` — state-mismatch detection, queue inspection, conversation summary, projects
//! - `policy`             — shared ToolPolicy JSON construction
//! - `remediation`        — self-healing batch operations
//! - `workflows`          — list workflows and dispatch history

pub mod agents;
pub mod approvals;
pub mod communicate;
pub mod creation;
pub mod diagnostic;
pub mod health;
pub mod knowledge;
pub mod lifecycle;
pub mod memory;
pub mod metrics;
pub mod notifications;
pub mod orchestrator_debug;
pub mod policy;
pub mod remediation;
pub mod workflows;
