# 0001 — agentd-mcp: MCP Observability Server via stdio Transport

**Date**: 2026-03-23
**Status**: Proposed
**Deciders**: architect agent, project owner
**Related**: #248, #249–#259

## Context

agentd needs an interface that allows MCP-capable clients (Claude Code, Claude
Desktop) to inspect, diagnose, and remediate issues across all agentd services.
The Model Context Protocol (MCP) is the natural fit — it provides structured
tool invocation over a standardized protocol.

Every existing agentd service follows a common template:

- HTTP server on an allocated port (170xx dev / 70xx prod)
- SeaORM + SQLite for persistence
- `agentd_common::server::init_tracing()` for structured logging
- `agentd_common::error::ApiError` for HTTP error responses

agentd-mcp deviates from this template in several ways, making it the first
service to break the established pattern. This ADR records why those deviations
are intentional and appropriate.

## Decision Drivers

- **Client compatibility**: Claude Code and Claude Desktop register MCP servers
  via stdio transport in their configuration files. An HTTP-based MCP server
  would require a separate proxy or adapter.
- **No persistent state**: agentd-mcp is a pass-through aggregation layer — it
  calls existing service REST APIs and returns structured results. There is
  nothing to persist locally.
- **Minimal coupling**: agentd-mcp should depend only on `agentd-common` for
  shared response types and `reqwest` for HTTP calls — the same dependency
  pattern as the CLI.
- **Ecosystem alignment**: The `rmcp` crate is the primary Rust implementation
  of the MCP protocol, providing server scaffolding, tool registration, and
  schema generation.

## Options Considered

### Option A: stdio transport with `rmcp` (pass-through, no storage)

agentd-mcp is a standalone binary that communicates with MCP clients over
stdin/stdout. It makes HTTP calls to agentd services using `reqwest` and returns
results as MCP tool responses. No port allocation, no database, no migrations.

- **Pros**:
  - Direct compatibility with Claude Code and Claude Desktop configuration
  - No port conflicts or allocation bookkeeping
  - No storage overhead for a stateless aggregation layer
  - Clean separation — agentd-mcp is an external interface, not an internal
    service
  - Tracing output goes to stderr, keeping stdout clean for MCP protocol
- **Cons**:
  - Breaks the standard service template (no port, no storage, no `ApiError`)
  - Cannot be health-checked by the existing `agent status` mechanism
  - `rmcp` is pre-1.0; API stability is not guaranteed
  - Testing requires MCP protocol harness rather than HTTP requests

### Option B: HTTP server exposing MCP-compatible endpoints

agentd-mcp runs as a standard HTTP service on a port (e.g., 17007/7007) and
implements MCP tool dispatch over HTTP. An adapter layer translates between
MCP protocol and HTTP.

- **Pros**:
  - Follows the established service template exactly
  - Can be health-checked via `GET /health`
  - Testing uses standard HTTP client patterns
- **Cons**:
  - Claude Code and Claude Desktop don't natively connect to HTTP MCP servers
    without an adapter
  - Adds an unnecessary network hop and port allocation for a client-side tool
  - Would need a stdio wrapper anyway for MCP client registration
  - Adds complexity without architectural benefit

### Option C: Embed MCP tools directly in the CLI

Instead of a separate crate, add MCP tool definitions to the existing `agent`
CLI binary with a `agent mcp-serve` subcommand.

- **Pros**:
  - No new crate; reuses existing CLI infrastructure
  - CLI already has HTTP clients for all services
- **Cons**:
  - Bloats the CLI binary with MCP SDK dependencies
  - Mixes two distinct interfaces (human CLI and machine MCP) in one binary
  - MCP server lifecycle is different from CLI command execution
  - Harder to version and deploy independently

## Decision

**Chosen**: Option A — stdio transport with `rmcp`, pass-through architecture

agentd-mcp is a pass-through MCP server that communicates via stdio and calls
existing service REST APIs over HTTP. It has no port allocation, no local
storage, and no database migrations.

This satisfies the primary driver (Claude Code/Desktop compatibility) while
maintaining minimal coupling to internal services. The deviation from the
standard service template is justified because agentd-mcp is fundamentally a
different kind of component — an external client interface, not an internal
service.

## Consequences

**Positive**:
- Zero-config MCP registration in Claude Code (`claude mcp add agentd-mcp`)
- No port conflicts or allocation coordination
- Stateless design means no migration burden and no data consistency concerns
- Clean dependency graph: `agentd-common` + `reqwest` + `rmcp`

**Negative / Trade-offs**:
- Cannot be monitored via the standard `agent status` health check flow
- `rmcp` pre-1.0 dependency introduces API stability risk; pin to a specific
  minor version and track upstream releases
- Integration testing requires an MCP protocol test harness rather than HTTP
  assertions
- New contributors must understand that agentd-mcp intentionally does not follow
  the standard service template

**Neutral**:
- 28-tool inventory is large but well-organized into phases; tools can be added
  incrementally without architectural changes
- Self-healing tools (Phase 4) have write side-effects; tool descriptions should
  include risk-level annotations so MCP clients present appropriate caution

## Implementation Notes

- **Crate location**: `crates/mcp/`
- **Binary name**: `agentd-mcp`
- **Dependencies**: `agentd-common` (shared types: `PaginatedResponse`,
  `HealthResponse`), `reqwest` (HTTP client), `rmcp` (MCP SDK), `schemars`
  (tool parameter schemas), `tokio`, `tracing`
- **Tracing**: Output to stderr via `tracing-subscriber` (stdout is reserved
  for MCP protocol). Do NOT call `agentd_common::server::init_tracing()` —
  configure a stderr-only subscriber directly.
- **Service URLs**: Resolve from environment variables (`AGENTD_*_SERVICE_URL`)
  following the same convention as the CLI and agent configs.
- **Shared types**: Import `PaginatedResponse`, `HealthResponse`, and other
  response types from `agentd-common`. Do NOT redefine them in the MCP crate.
- **`rmcp` version strategy**: Pin to `0.16.x` (or current stable minor).
  Track the `rmcp` changelog for breaking changes. If `rmcp` proves unstable,
  the stdio protocol is simple enough to implement directly as a fallback.
- **Documentation**: Update `docs/public/install.md` to list agentd-mcp
  (noting stdio transport, no port). Add MCP client configuration guide as
  part of #259.
