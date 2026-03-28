//! agentd-mcp library — exposes internal modules for integration testing.
//!
//! The binary entry point is `main.rs`. This lib re-exports the key modules
//! so that integration tests in `tests/` can access `AgentdClient`,
//! `AgentdMcpConfig`, and the tool implementation functions.

use anyhow::Result;
use tracing::info;

pub mod client;
pub mod config;
pub mod server;
pub mod tools;

/// Start the MCP server on stdio transport.
///
/// This reads configuration from environment variables, creates the server,
/// and blocks until the client disconnects. All tracing output goes to stderr
/// so it does not interfere with the MCP JSON-RPC framing on stdout.
pub async fn run(config: config::AgentdMcpConfig) -> Result<()> {
    info!("Starting agentd-mcp server on stdio");

    let server = server::AgentdMcp::new(config);
    let transport = rmcp::transport::stdio();
    let running = rmcp::serve_server(server, transport).await?;
    running.waiting().await?;

    Ok(())
}
