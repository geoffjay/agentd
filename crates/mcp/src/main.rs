//! agentd-mcp — MCP server entry point.
//!
//! Starts an MCP server on stdio using the rmcp framework. The server
//! exposes agentd services as tools for Claude and other MCP clients.
//!
//! # Running
//!
//! ```bash
//! cargo run -p agentd-mcp
//! ```
//!
//! # Environment Variables
//!
//! | Variable                  | Default                   | Description               |
//! |---------------------------|---------------------------|---------------------------|
//! | `RUST_LOG`                | `info`                    | Log level (stderr)        |
//! | `AGENTD_ORCHESTRATOR_URL` | `http://127.0.0.1:17000` | Orchestrator service URL  |
//! | `AGENTD_COMMUNICATE_URL`  | `http://127.0.0.1:17010` | Communicate service URL   |
//! | `AGENTD_MEMORY_URL`       | `http://127.0.0.1:17008` | Memory service URL        |
//! | `AGENTD_NOTIFY_URL`       | `http://127.0.0.1:17001` | Notify service URL        |
//! | `AGENTD_ASK_URL`          | `http://127.0.0.1:17002` | Ask service URL           |
//! | `AGENTD_WRAP_URL`         | `http://127.0.0.1:17003` | Wrap service URL          |
//!
//! # MCP Transport
//!
//! This server uses stdio transport (stdin/stdout). All tracing output is
//! directed to **stderr** so it does not interfere with the MCP JSON-RPC
//! framing on stdout.

use agentd_mcp::config::AgentdMcpConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Write tracing to stderr — stdout is reserved for the MCP JSON-RPC transport.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    agentd_mcp::run(AgentdMcpConfig::load()).await
}
