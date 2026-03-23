# MCP Observability Server (agentd-mcp) — Detailed Plan

## Overview

agentd-mcp is an MCP (Model Context Protocol) server that exposes tools for
inspecting, troubleshooting, diagnosing, and remediating issues across all agentd
services. It communicates with MCP clients (Claude Code, Claude Desktop) over
stdin/stdout using the stdio transport, and calls existing agentd service REST
APIs over HTTP.

The primary use-case is a **self-healing loop**: an MCP client can call
`diagnose_system`, discover which agents have failed or which dispatches are
stale, invoke the appropriate remediation tools, and verify that the system
returned to a healthy state — all through structured MCP tool calls without
human intervention.

## Design Decisions

See **[ADR 0001](./decisions/0001-agentd-mcp-stdio-transport.md)** for the
architectural decision record. Key choices:

- **stdio transport** — required for zero-configuration registration with Claude
  Code (`claude mcp add agentd-mcp`) and Claude Desktop. No HTTP server, no port
  allocation.
- **Pass-through architecture** — agentd-mcp is a stateless aggregation layer. It
  holds no database and persists nothing locally; all state lives in the existing
  services.
- **`rmcp` SDK** — the primary Rust implementation of the MCP protocol; provides
  server scaffolding, tool registration, and JSON schema generation.
- **Deliberately breaks the standard service template** — no `axum`, no SeaORM,
  no `ApiError`, no `init_tracing()`. This is intentional and documented.

## Technology Stack

- **MCP SDK**: `rmcp` v0.16.x (pin to minor, track upstream)
- **HTTP Client**: `reqwest` (workspace dependency)
- **Schema Generation**: `schemars` 0.8 (for MCP tool parameter JSON schemas)
- **Async Runtime**: `tokio` (workspace dependency)
- **Tracing**: `tracing` + `tracing-subscriber` — output to **stderr only**;
  stdout is reserved for MCP protocol messages
- **Shared Types**: `agentd-common` (`PaginatedResponse`, `HealthResponse`)

### Cargo.toml

```toml
[package]
name = "mcp"
version.workspace = true
edition.workspace = true

[[bin]]
name = "agentd-mcp"
path = "src/main.rs"

[dependencies]
tokio        = { workspace = true }
tracing      = { workspace = true }
anyhow       = { workspace = true }
serde        = { workspace = true }
serde_json   = { workspace = true }
reqwest      = { workspace = true, features = ["json"] }
chrono       = { workspace = true }
uuid         = { version = "1.11", features = ["v4", "serde"] }
agentd-common = { path = "../common" }

# MCP SDK (stdio transport)
rmcp = { version = "0.16", features = ["server", "transport-io"] }

# JSON schema generation for tool parameters
schemars = "0.8"

tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

[dev-dependencies]
tokio-test = "0.4"
```

## Architecture

### Crate Location

`crates/mcp/` — binary-only crate (no `lib.rs`). The binary name is
`agentd-mcp`.

### Project Structure

```
crates/mcp/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point: initialise tracing, build server, run stdio
    ├── client.rs            # ServiceClient — HTTP client for all agentd services
    ├── error.rs             # MCP-compatible error type
    ├── server.rs            # AgentdMcpServer struct + rmcp ServerHandler impl
    └── tools/
        ├── mod.rs           # Re-exports all tool modules
        ├── inspection.rs    # list_agents, get_agent, get_agent_status_summary,
        │                    # list_workflows, get_workflow, list_dispatches,
        │                    # get_failed_dispatches, list_notifications,
        │                    # get_notification, get_actionable_notifications,
        │                    # list_pending_approvals, get_agent_approvals
        ├── health.rs        # check_service_health, check_single_service,
        │                    # get_system_metrics, get_prometheus_metrics
        ├── management.rs    # restart_agent, send_agent_message,
        │                    # update_agent_tool_policy, terminate_agent,
        │                    # update_agent_model, approve_tool_request,
        │                    # deny_tool_request, create_notification,
        │                    # dismiss_notification
        └── diagnostics.rs   # diagnose_agent, diagnose_workflow, diagnose_system,
                             # check_connectivity, restart_failed_agents,
                             # retry_failed_dispatches, cleanup_stale_dispatches,
                             # auto_approve_safe_tools, resolve_notification_backlog
```

### Tracing Initialisation

agentd-mcp **must not** call `agentd_common::server::init_tracing()` — that
function writes JSON to stdout, which would corrupt the MCP protocol stream.

Instead, configure a stderr-only subscriber directly in `main.rs`:

```rust
tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter(
        tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()),
    )
    .init();
```

### Service URL Resolution

All service URLs are read from environment variables, following the same
convention as the CLI and agent configs:

| Variable                          | Default                     | Service     |
|-----------------------------------|-----------------------------|-------------|
| `AGENTD_ORCHESTRATOR_SERVICE_URL` | `http://localhost:7006`     | orchestrator|
| `AGENTD_NOTIFY_SERVICE_URL`       | `http://localhost:7004`     | notify      |
| `AGENTD_WRAP_SERVICE_URL`         | `http://localhost:7005`     | wrap        |
| `AGENTD_MONITOR_SERVICE_URL`      | `http://localhost:7003`     | monitor     |
| `AGENTD_ASK_SERVICE_URL`          | `http://localhost:7001`     | ask         |
| `AGENTD_HOOK_SERVICE_URL`         | `http://localhost:7002`     | hook        |

### ServiceClient

`src/client.rs` provides a single `ServiceClient` struct that wraps a
`reqwest::Client` and exposes typed methods for each service:

```rust
pub struct ServiceClient {
    client: reqwest::Client,
    orchestrator: String,
    notify: String,
    wrap: String,
    monitor: String,
    ask: String,
    hook: String,
}

impl ServiceClient {
    pub fn from_env() -> anyhow::Result<Self> { /* read env vars */ }

    // Orchestrator
    pub async fn list_agents(&self, ...) -> anyhow::Result<PaginatedResponse<AgentResponse>> {}
    pub async fn get_agent(&self, id: Uuid) -> anyhow::Result<AgentResponse> {}
    // ... (one method per tool, calling the existing REST API)
}
```

### MCP Server Wiring

`src/server.rs` implements `rmcp::ServerHandler` for `AgentdMcpServer`:

```rust
pub struct AgentdMcpServer {
    client: Arc<ServiceClient>,
}

impl rmcp::ServerHandler for AgentdMcpServer {
    fn get_info(&self) -> ServerInfo { /* name, version, description */ }
    fn list_tools(&self) -> Vec<Tool> { /* 28 tool descriptors */ }
    async fn call_tool(&self, req: CallToolRequest) -> CallToolResult { /* dispatch */ }
}
```

`src/main.rs` builds the server and runs the stdio transport:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_stderr_tracing();
    let client = Arc::new(ServiceClient::from_env()?);
    let server = AgentdMcpServer::new(client);
    rmcp::stdio::serve(server).await?;
    Ok(())
}
```

---

## Tool Inventory (28 tools)

### Category 1 — Inspection (read-only, 12 tools)

All tools in this category issue `GET` requests to the orchestrator or notify
service.

| Tool | HTTP call | Description |
|------|-----------|-------------|
| `list_agents` | `GET /agents?status=<filter>&limit=<n>&offset=<n>` | List agents with optional status filter |
| `get_agent` | `GET /agents/{id}` | Full agent details including config, policy, env |
| `get_agent_status_summary` | `GET /agents` (aggregate) | Fleet-wide status counts (running/stopped/failed) |
| `list_workflows` | `GET /workflows?limit=<n>&offset=<n>` | List all workflows with metadata |
| `get_workflow` | `GET /workflows/{id}` | Full workflow config including prompt template |
| `list_dispatches` | `GET /workflows/{id}/history?limit=<n>&offset=<n>` | Dispatch history for a workflow |
| `get_failed_dispatches` | `GET /workflows` + per-workflow history | Cross-workflow failure report (aggregated) |
| `list_notifications` | `GET /notifications?status=<f>&priority=<p>` | Notifications with status/priority filters |
| `get_notification` | `GET /notifications/{id}` | Full notification details |
| `get_actionable_notifications` | `GET /notifications?status=pending` | Pending notifications requiring response |
| `list_pending_approvals` | `GET /approvals?status=pending` | All pending tool approval requests |
| `get_agent_approvals` | `GET /approvals?agent_id={id}` | Approvals for a specific agent |

#### Tool Input Schemas (examples)

```rust
#[derive(JsonSchema, Deserialize)]
pub struct ListAgentsInput {
    /// Filter by status: "running", "stopped", "failed", or omit for all.
    pub status: Option<String>,
    /// Maximum number of results (default: 50, max: 200).
    pub limit: Option<usize>,
    /// Offset for pagination (default: 0).
    pub offset: Option<usize>,
}

#[derive(JsonSchema, Deserialize)]
pub struct GetAgentInput {
    /// UUID of the agent to retrieve.
    pub agent_id: String,
}
```

---

### Category 2 — Health & Metrics (4 tools)

| Tool | HTTP calls | Description |
|------|-----------|-------------|
| `check_service_health` | `GET /health` on all 6 services (concurrent) | Full fleet health check with latency |
| `check_single_service` | `GET /health` on named service | Health check a specific service |
| `get_system_metrics` | `GET /metrics` on monitor service | CPU, memory, disk, load averages |
| `get_prometheus_metrics` | `GET /metrics/prometheus` on any service | Parsed Prometheus counters/gauges |

`check_service_health` issues all 6 health checks concurrently via
`tokio::join!` and returns a structured summary:

```json
{
  "orchestrator": { "status": "ok", "version": "0.10.0", "latency_ms": 3 },
  "notify":       { "status": "ok", "version": "0.10.0", "latency_ms": 2 },
  "wrap":         { "status": "degraded", "reason": "database unreachable", "latency_ms": 45 },
  ...
}
```

---

### Category 3 — Management (write, 9 tools)

These tools issue POST/PUT/DELETE requests and have side effects. Tool
descriptions must include a `⚠️ write operation` annotation so MCP clients can
surface appropriate caution.

| Tool | HTTP call | Description |
|------|-----------|-------------|
| `restart_agent` | `POST /agents` (re-create) | Restart a failed/stopped agent |
| `send_agent_message` | `POST /agents/{id}/message` | Send a prompt to a running agent |
| `update_agent_tool_policy` | `PUT /agents/{id}/policy` | Change an agent's tool policy |
| `terminate_agent` | `DELETE /agents/{id}` | Kill an agent's tmux session |
| `update_agent_model` | `PUT /agents/{id}/model` | Switch an agent's model |
| `approve_tool_request` | `POST /approvals/{id}/approve` | Approve a pending tool use |
| `deny_tool_request` | `POST /approvals/{id}/deny` | Deny a pending tool use |
| `create_notification` | `POST /notifications` | Create a system notification |
| `dismiss_notification` | `DELETE /notifications/{id}` | Dismiss a notification |

---

### Category 4 — Diagnostics & Self-Healing (composite, 9 tools)

These tools compose multiple API calls into higher-level operations.

#### Diagnostic tools (read-only aggregation)

| Tool | Logic | Description |
|------|-------|-------------|
| `diagnose_agent` | get agent + recent dispatches + approvals | Multi-point agent health analysis |
| `diagnose_workflow` | get workflow + full history + agent status | Dispatch pattern analysis |
| `diagnose_system` | all agents + all workflows + service health | Full system diagnostic with prioritized issues |
| `check_connectivity` | health check all services + external DNS | Inter-service and external connectivity test |

`diagnose_system` output format:

```json
{
  "issues": [
    { "severity": "critical", "description": "2 agents in failed state", "agents": ["..."] },
    { "severity": "warning",  "description": "5 dispatches stale >1h",   "workflows": ["..."] },
    { "severity": "info",     "description": "30 notifications older than 7 days" }
  ],
  "summary": "🔴 2 critical, 🟡 1 warning, 🟢 1 info"
}
```

#### Self-healing tools (write, bulk operations)

| Tool | Logic | Description |
|------|-------|-------------|
| `restart_failed_agents` | list failed → restart each | Bulk restart all failed agents |
| `retry_failed_dispatches` | list failed dispatches → send_agent_message for each | Re-send prompts for failed dispatches |
| `cleanup_stale_dispatches` | list dispatches in_progress >1h → mark failed | Mark stale in-progress dispatches as failed |
| `auto_approve_safe_tools` | list pending approvals → approve read-only ones | Auto-approve bash/read tool requests |
| `resolve_notification_backlog` | list old pending notifications → dismiss | Bulk dismiss expired notifications |

---

## Implementation Phases

Issue #248 is the tracking issue for the following implementation sub-issues.
Each sub-issue corresponds to a milestone-gated implementation phase.

### Phase 1: Foundation (#249)

**Deliverable**: A compiling, runnable `agentd-mcp` binary with stdio transport
that registers with Claude Code and responds to `list_tools`.

Tasks:
- [ ] Create `crates/mcp/` with `Cargo.toml`
- [ ] Add `mcp` to workspace `Cargo.toml` members
- [ ] Implement `ServiceClient::from_env()` with URL resolution
- [ ] Implement `AgentdMcpServer` skeleton (empty tool list)
- [ ] Wire stdio transport in `main.rs`
- [ ] Configure stderr-only tracing
- [ ] `cargo check` and `cargo clippy` pass

### Phase 2: Inspection Tools (#250–#253, parallelizable)

These sub-issues can be developed in parallel once #249 is merged.

**#250 — Agent inspection tools**
- `list_agents`, `get_agent`, `get_agent_status_summary`
- Input: `ListAgentsInput`, `GetAgentInput`
- Output: formatted agent details as MCP text content

**#251 — Workflow and dispatch inspection tools**
- `list_workflows`, `get_workflow`, `list_dispatches`, `get_failed_dispatches`
- Aggregates dispatch history across all workflows for `get_failed_dispatches`

**#252 — Notification and approval inspection tools**
- `list_notifications`, `get_notification`, `get_actionable_notifications`
- `list_pending_approvals`, `get_agent_approvals`

**#253 — Service health and system metrics tools**
- `check_service_health` (concurrent tokio::join! across 6 services)
- `check_single_service`, `get_system_metrics`, `get_prometheus_metrics`

### Phase 3: Management Tools (#254–#255, parallelizable)

**#254 — Agent lifecycle management**
- `restart_agent`, `send_agent_message`, `update_agent_tool_policy`
- `terminate_agent`, `update_agent_model`
- All require `⚠️ write operation` in tool description

**#255 — Approval and notification management**
- `approve_tool_request`, `deny_tool_request`
- `create_notification`, `dismiss_notification`

### Phase 4: Intelligence Layer (#256–#257, sequential)

**#256 — Diagnostic tools** (depends on #250–#253)
- `diagnose_agent`, `diagnose_workflow`, `diagnose_system`, `check_connectivity`
- Each composes multiple inspection calls into a structured analysis

**#257 — Self-healing remediation tools** (depends on #254–#256)
- `restart_failed_agents`, `retry_failed_dispatches`, `cleanup_stale_dispatches`
- `auto_approve_safe_tools`, `resolve_notification_backlog`
- All return structured summaries of actions taken

### Phase 5: Quality & Documentation (#258–#259)

**#258 — Integration tests**
- Mock `ServiceClient` with `mockito` or `wiremock`
- Test each tool with success and error responses
- Test concurrent health check logic
- Test diagnostic aggregation logic

**#259 — Documentation and client configuration guide**
- `docs/public/mcp-guide.md`: Claude Code registration, Claude Desktop config
- Update `docs/public/install.md` to list `agentd-mcp` (stdio, no port)
- Tool descriptions with examples for each of the 28 tools
- Self-healing workflow walkthrough

### Dependency Graph

```
#249 (scaffold)
 ├── #250 (agent inspection)   ─┐
 ├── #251 (workflow inspection)  ├── #256 (diagnostics) ──→ #257 (self-healing)
 ├── #252 (notifications)        │
 ├── #253 (health/metrics)      ─┘
 ├── #254 (agent mgmt)       ───────────────────────────→ #257 (self-healing)
 └── #255 (approvals/notif)  ───────────────────────────→ #257 (self-healing)
                                                               │
                                                    ┌──────────┴──────────┐
                                                    #258 (tests)     #259 (docs)
```

---

## MCP Client Registration

### Claude Code

```bash
claude mcp add agentd-mcp
# Or manually:
claude mcp add --name agentd-mcp /path/to/agentd-mcp
```

Or via `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "agentd-mcp": {
      "command": "agentd-mcp",
      "env": {
        "AGENTD_ORCHESTRATOR_SERVICE_URL": "http://localhost:7006",
        "AGENTD_NOTIFY_SERVICE_URL": "http://localhost:7004",
        "AGENTD_WRAP_SERVICE_URL": "http://localhost:7005",
        "AGENTD_MONITOR_SERVICE_URL": "http://localhost:7003",
        "AGENTD_ASK_SERVICE_URL": "http://localhost:7001",
        "AGENTD_HOOK_SERVICE_URL": "http://localhost:7002"
      }
    }
  }
}
```

### Claude Desktop

```json
{
  "mcpServers": {
    "agentd-mcp": {
      "command": "/usr/local/bin/agentd-mcp",
      "env": {
        "AGENTD_ORCHESTRATOR_SERVICE_URL": "http://localhost:7006"
      }
    }
  }
}
```

(Claude Desktop config file: `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS.)

---

## Testing Strategy

### Unit Tests (per-tool module)

Each tool function should have unit tests with a mock `ServiceClient`. Use
`wiremock` or `mockito` to serve canned HTTP responses:

```rust
#[tokio::test]
async fn list_agents_returns_formatted_output() {
    let mock = MockServer::start().await;
    Mock::given(method("GET")).and(path("/agents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&canned_response()))
        .mount(&mock).await;

    let client = ServiceClient::with_base_url(&mock.uri());
    let result = list_agents(&client, ListAgentsInput::default()).await.unwrap();
    assert!(result.contains("running"));
}
```

### Integration Tests (#258)

- Verify all 28 tools compile and produce valid MCP tool descriptors
- Test the concurrent health-check logic with simulated service latency
- Test diagnostic aggregation: inject known failure states, verify prioritized output
- Test self-healing tools: verify they only act on the expected set of resources

### MCP Protocol Conformance

The `rmcp` crate handles protocol-level conformance. Verify:
- `list_tools` returns all 28 tools with valid JSON schemas
- `call_tool` returns `CallToolResult` with `isError: false` on success
- `call_tool` returns `CallToolResult` with `isError: true` + message on service errors
- Tool input validation rejects invalid UUIDs and unknown parameter names

### Error Handling

All tool implementations must handle service unavailability gracefully:

```rust
// Pattern for all tool call implementations:
match client.list_agents(input).await {
    Ok(result) => CallToolResult::text(format_result(result)),
    Err(e) => CallToolResult::error(format!("Service unavailable: {}", e)),
}
```

Never panic or return an `Err` from `call_tool` — always return a
`CallToolResult` with `isError: true` for client errors.

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `rmcp` API breaks between minor versions | Medium | High | Pin to exact minor; add `CHANGELOG` tracking in `Cargo.toml` comment |
| Service unavailability during bulk operations | High | Medium | Per-operation error handling; partial success reporting |
| Tool count (28) makes `list_tools` response large | Low | Low | MCP protocol handles large tool lists; no action needed |
| Self-healing tools cause unintended side effects | Low | High | Risk-level annotations in descriptions; require explicit confirmation in diagnostics output |
| Stdout pollution corrupts MCP stream | Medium | Critical | All logging via `tracing` to stderr; never `println!` in tool code |

---

## Non-Goals

The following are explicitly **out of scope** for agentd-mcp:

- **HTTP server mode**: agentd-mcp is stdio-only. No port, no HTTP.
- **Local state**: No database, no configuration files, no migrations.
- **Agent SDK connection**: agentd-mcp does not connect via WebSocket to the
  orchestrator. It only calls REST APIs.
- **Real-time streaming**: Tool results are synchronous request/response. No
  streaming tool outputs.
- **Multi-tenant or remote deployments**: Designed for local use only (all
  service URLs point to localhost by default).

---

## Milestone

All sub-issues (#249–#259) are assigned to **v0.4.1 — MCP Observability Server**.

The tracking issue for this plan is **#248**.
