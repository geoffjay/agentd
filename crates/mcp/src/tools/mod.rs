//! MCP tool implementations for agentd services.
//!
//! Tools are grouped by the agentd service they interact with.
//!
//! # Tool Modules
//!
//! - `agents`             — list/inspect agents
//! - `approvals`          — tool approval requests
//! - `communicate`        — rooms, participants, messages
//! - `diagnostic`         — cross-service diagnostics (system, agent, workflow)
//! - `health`             — service health probes, system metrics, prometheus
//! - `lifecycle`          — restart/terminate agents, send messages, update policy/model
//! - `memory`             — search and list stored memories
//! - `notifications`      — list/create/dismiss notifications
//! - `orchestrator_debug` — state-mismatch detection, queue inspection, conversation summary, projects
//! - `remediation`        — self-healing batch operations
//! - `workflows`          — list workflows and dispatch history

pub mod agents;
pub mod approvals;
pub mod communicate;
pub mod diagnostic;
pub mod health;
pub mod lifecycle;
pub mod memory;
pub mod notifications;
pub mod orchestrator_debug;
pub mod remediation;
pub mod workflows;
