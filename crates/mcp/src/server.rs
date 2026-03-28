//! MCP server implementation for agentd.
//!
//! `AgentdMcp` is the central server struct. It implements `ServerHandler`
//! via rmcp's `#[tool(tool_box)]` macro, which wires the tool routing into
//! the MCP `tools/list` and `tools/call` handlers.
//!
//! Tools are added in subsequent issues — this module provides the empty
//! scaffold that subsequent issues populate.

use crate::client::AgentdClient;
use crate::config::AgentdMcpConfig;
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};
use std::sync::Arc;

/// The agentd MCP server.
///
/// Exposes agentd services (orchestrator, communicate, memory, notify, ask,
/// wrap) as MCP tools that Claude and other MCP clients can call.
#[derive(Debug, Clone)]
pub struct AgentdMcp {
    #[allow(dead_code)]
    client: AgentdClient,
}

#[tool(tool_box)]
impl AgentdMcp {
    /// Create a new server instance from the given configuration.
    pub fn new(config: AgentdMcpConfig) -> Self {
        let config = Arc::new(config);
        Self { client: AgentdClient::new(config) }
    }
}

#[tool(tool_box)]
impl ServerHandler for AgentdMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "agentd MCP server — exposes agentd agent management, \
                 messaging, memory, notification, approval, and workflow \
                 services as MCP tools."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
