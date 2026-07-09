# 0002 — agentd Agent Protocol (AAP): Vendor-Neutral Agent Communication

**Date**: 2026-07-09
**Status**: Proposed
**Deciders**: project owner, architect agent

## Context

agentd launches and drives AI coding agents as subprocesses, but the communication path is Claude Code
specific end to end:

- The orchestrator hardcodes `agent_type: "claude-code"` / `model_provider: "anthropic"` at both spawn
  sites (`crates/orchestrator/src/manager.rs`).
- `build_claude_command` hardcodes the `claude` binary and every Claude CLI flag (`--print --verbose
  --output-format stream-json --input-format stream-json --permission-prompt-tool stdio`, `--sdk-url`,
  `--mcp-config`, `--append-system-prompt`, `--worktree`, `--add-dir`, `--model`).
- The Claude Agent SDK `initialize` handshake is hardcoded (`make_initialize_line`).
- Inbound messages are duck-typed: parsed as `serde_json::Value` and string-matched on `type` in
  `handle_incoming_message` (`crates/orchestrator/src/websocket.rs`). There is no typed, versioned
  protocol.

We want any agent developer to be able to integrate their agent with agentd without modifying agentd,
and we do not want Claude Code to be a first-class citizen.

## Decision Drivers

- **Openness**: a third party should implement one contract, in any language, and their agent works with
  agentd.
- **Vendor neutrality**: no single agent's message shape should leak into the core.
- **Preserve existing value**: agentd already normalizes agent output into an internal vocabulary
  (`ConversationEventType`) and has mature streaming, persistence, tool-policy, and approval machinery.
  Keep it.
- **Separation of concerns**: *where* a process runs (execution backend) is orthogonal to *how* agentd
  talks to it (the protocol).

## Options Considered

### Option A: External wire protocol with adapter processes (chosen)

Define AAP, a language-agnostic NDJSON protocol. Each agent is fronted by an **adapter** process that
translates AAP to and from its native format. Ship a reference adapter for Claude Code.

- **Pros**: implementable in any language; core carries no vendor specifics; Claude Code is just one
  adapter; adapters version independently.
- **Cons**: one extra (thin) translator process per agent; agent configuration must flow as data over
  the protocol rather than as launch flags.

### Option B: In-process Rust `Provider` trait

Realize the abstraction as a Rust trait compiled into agentd; each provider is a Rust module.

- **Pros**: no extra process.
- **Cons**: only extensible by editing and recompiling agentd; not implementable by outside developers —
  fails the primary driver.

## Decision

**Chosen**: Option A. AAP is the contract (see `docs/spec/agent-protocol-v1.md`). Adapters are separate
processes speaking AAP over a mandatory stdio binding (and an optional websocket binding for
tmux/docker backends). Claude Code ships as the in-tree reference adapter `agentd-adapter-claude`.

Rollout is a **hard cut**: the hardcoded Claude launch and protocol path is removed from the orchestrator
and replaced by AAP plus the Claude adapter. There is no dual path.

## Consequences

**Positive**:
- Third parties integrate any agent by implementing one documented protocol.
- The orchestrator's inbound message handling becomes typed instead of duck-typed.
- Configuration flows as structured `initialize` data; the adapter owns native argv.

**Negative / Trade-offs**:
- A thin adapter process is added to the agent launch path.
- The `crates/orchestrator/src/` core rewire is a human-approval-gated area
  (`docs/planning/autonomous-pipeline-gates.md`) and must be reviewed.
- Capability negotiation introduces documented degradation paths (e.g. no `tool_approval` disables
  approval holds) that must be tested per adapter.

**Neutral**:
- The `ExecutionBackend` abstraction (tmux/docker/pty/subprocess) is unchanged; AAP rides on top.
- Interactive PTY mode (human types in a terminal) is out of AAP's scope and unchanged.

## Implementation Notes

- **Spec**: `docs/spec/agent-protocol-v1.md`.
- **Protocol crate**: `crates/agent-protocol/` (`agentd-agent-protocol`) — typed `HostMessage` /
  `AgentMessage` serde enums, shared by the orchestrator and adapters.
- **Reference adapter**: `crates/adapter-claude/` (`agentd-adapter-claude`) — new home for the logic
  currently in `build_claude_command`, `make_initialize_line`, and the Claude stream-json translation.
- **Seam**: add `agent_type` to `AgentConfig` (default `"claude"`); resolve `agent_type` → adapter argv.
- **Transports**: stdio (mandatory), websocket (optional); selected via `AGENTD_AAP_TRANSPORT` /
  `AGENTD_AAP_WS_URL`.
