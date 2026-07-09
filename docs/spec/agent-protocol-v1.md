# agentd Agent Protocol (AAP) — v1

Status: **Draft**
Protocol version: **1**

## 1. Purpose and scope

The agentd Agent Protocol (AAP) is a vendor-neutral wire protocol for communication between the agentd
orchestrator (the **host**) and an AI coding agent (the **agent**). It defines how a host launches an
agent, exchanges prompts and responses, streams incremental output, negotiates tool-use approval, and
reports usage.

AAP is deliberately **not** tied to any single vendor. An agent developer implements AAP in an
**adapter** — a process that translates AAP to and from their agent's native format. agentd ships a
reference adapter for Claude Code (`agentd-adapter-claude`), but Claude Code holds no privileged
position: it is one adapter among many.

AAP governs the **programmatic communication path** only. It does **not** govern:

- **Where** the agent process runs. That is the concern of the host's execution backend
  (subprocess, tmux, docker, pty). AAP rides on top of whichever backend is active.
- **Interactive PTY mode**, in which a human types directly into a terminal running an agent. That path
  bypasses AAP entirely.

## 2. Roles and terminology

| Term | Meaning |
| --- | --- |
| Host | The agentd orchestrator. Owns policy, persistence, and streaming to end users. |
| Agent | An AI coding agent, driven through an adapter. |
| Adapter | A process implementing AAP that wraps a specific agent. |
| Turn | One prompt and the agent's complete response to it, identified by a `turn_id`. |
| Tool call | A request by the agent to invoke a tool, identified by a `call_id`. |

Direction notation: **H→A** is host-to-agent (adapter stdin / host-sent), **A→H** is agent-to-host
(adapter stdout / host-received).

## 3. Framing

- The wire format is **newline-delimited JSON (NDJSON)**: exactly one JSON object per line, terminated
  by a single `\n` (U+000A). Objects MUST NOT contain unescaped newlines.
- Encoding is **UTF-8**.
- Every message is a JSON object with a top-level string field **`type`** that discriminates the
  message.
- Receivers **MUST ignore unknown fields** on a known message type (forward compatibility).
- Receivers **MUST NOT** treat an unknown `type` as fatal: log it and skip the line.
- Empty and whitespace-only lines MUST be ignored.

## 4. Transports

An adapter MUST support the stdio binding and MAY support the websocket binding. The host selects the
binding at launch and communicates the choice through environment variables.

### 4.1 stdio (mandatory baseline)

- Host→agent AAP frames are written to the adapter's **stdin**.
- Agent→host AAP frames are written to the adapter's **stdout**.
- The adapter's **stderr** is reserved for human-readable logs and MUST NOT carry AAP frames.
- Used by the `subprocess` and `pty` execution backends.
- Environment: `AGENTD_AAP_TRANSPORT=stdio`.

### 4.2 websocket (optional)

- The adapter dials back to a host-provided WebSocket URL and exchanges the same AAP frames as text
  messages (one JSON object per WebSocket text frame; the trailing `\n` is optional over WebSocket).
- Used by the `tmux` and `docker` execution backends, where clean stdio piping is not available.
- Environment: `AGENTD_AAP_TRANSPORT=websocket`, `AGENTD_AAP_WS_URL=ws://host:port/path`.

The message schema is identical across bindings; only the byte transport differs.

## 5. Invocation contract

1. The host resolves an **adapter command** (argv + environment) from the agent's configured
   `agent_type`.
2. The host launches that command via the active execution backend, injecting the AAP transport
   environment variables (§4).
3. AAP prescribes **no** command-line flags for the underlying agent. The adapter owns native argv
   construction.
4. All per-agent configuration is delivered in the `initialize` message (§6.1), **not** via argv. This
   is the central inversion of the pre-AAP design: configuration flows as data over the protocol, and
   the adapter maps it to native invocation.

## 6. Message reference

### 6.1 Handshake

#### `initialize` (H→A)

Sent once, first, before any other host message. Carries all agent configuration.

```json
{
  "type": "initialize",
  "protocol_version": 1,
  "model": "claude-sonnet-5",
  "system_prompt": { "mode": "replace", "text": "You are ...", "path": null },
  "workspace": { "cwd": "/repo", "additional_dirs": ["/other"], "worktree": false },
  "tools": {
    "mcp_servers": {
      "agentd": { "command": "agent", "args": ["mcp"], "env": { "AGENTD_CORE_URL": "http://..." } }
    }
  },
  "resume_token": null
}
```

- `protocol_version` (integer, required): the AAP version the host speaks. The adapter MUST refuse
  (emit a fatal `error`) if it cannot speak this version.
- `model` (string, optional): requested model. If omitted, the adapter uses its default.
- `system_prompt` (object, optional): `mode` is `"replace"` or `"append"`. Exactly one of `text` or
  `path` is set.
- `workspace.cwd` (string, required): working directory.
- `workspace.additional_dirs` (array of string, optional): extra directories the agent may access.
- `workspace.worktree` (boolean, optional): run in an isolated worktree if supported.
- `tools.mcp_servers` (object, optional): MCP server definitions, keyed by name. Each is
  `{ command, args, env }`.
- `resume_token` (string, optional): an opaque token from a prior `turn_complete` used to resume a
  conversation (capability `resume`).

#### `ready` (A→H)

Sent once the adapter has started its native agent and is prepared to accept prompts.

```json
{
  "type": "ready",
  "protocol_version": 1,
  "agent": { "name": "claude-code", "version": "2.1.x" },
  "capabilities": ["streaming", "thinking", "tool_approval", "usage_reporting",
                   "cost_reporting", "context_clear", "cancel", "mcp", "system_prompt_append"],
  "models": ["claude-sonnet-5", "claude-opus-4-8"]
}
```

- `capabilities` (array of string, required): the capability tokens the adapter supports (§7).
- `models` (array of string, optional): models the adapter can serve.

The host **MUST NOT** send a `prompt` before receiving `ready`.

### 6.2 Turn input (H→A)

#### `prompt`

```json
{ "type": "prompt", "turn_id": "t1", "content": "Refactor the parser." }
```

- `content` may be a string or an array of content blocks (`[{"type":"text","text":"..."}]`).
- The host assigns a unique `turn_id`; the adapter echoes it on all output for that turn.

#### `cancel` (capability `cancel`)

```json
{ "type": "cancel", "turn_id": "t1" }
```

Requests interruption of the active turn. If `turn_id` is omitted, cancels the current turn.

#### `clear_context` (capability `context_clear`)

```json
{ "type": "clear_context" }
```

Discards conversation history and starts a fresh context. The host increments its session counter on
acknowledgement.

#### `shutdown`

```json
{ "type": "shutdown" }
```

Requests graceful termination. The adapter tears down its native agent, flushes output, and exits.

### 6.3 Turn output (A→H)

#### `message`

```json
{
  "type": "message",
  "turn_id": "t1",
  "content": [
    { "type": "thinking", "text": "The parser is in src/parse.rs ..." },
    { "type": "text", "text": "I'll start by extracting the tokenizer." }
  ]
}
```

Assistant output blocks. `text` blocks are visible output; `thinking` blocks are reasoning
(capability `thinking`). Adapters MAY stream multiple `message` frames per turn.

#### `tool_call`

```json
{ "type": "tool_call", "turn_id": "t1", "call_id": "c1", "name": "Bash", "input": { "command": "ls" } }
```

Announces a tool invocation. `call_id` is unique within the turn and correlates with an
`approval_request` (if any).

#### `turn_complete`

```json
{
  "type": "turn_complete",
  "turn_id": "t1",
  "is_error": false,
  "stop_reason": "end_turn",
  "result_text": "Done.",
  "resume_token": null,
  "usage": {
    "input_tokens": 1200, "output_tokens": 340,
    "cache_read_input_tokens": 800, "cache_creation_input_tokens": 0,
    "total_cost_usd": 0.0123, "num_turns": 3,
    "duration_ms": 5000, "duration_api_ms": 4200
  }
}
```

Marks the end of a turn. `usage` is present only if the adapter has the `usage_reporting` capability
(`total_cost_usd` only with `cost_reporting`). `resume_token` is present only with the `resume`
capability.

#### `status`

```json
{ "type": "status", "state": "busy" }
```

Activity transitions. `state` is `"busy"` or `"idle"`.

#### `log`

```json
{ "type": "log", "level": "info", "message": "spawned claude pid=1234" }
```

Structured diagnostics. `level` is `"info"`, `"warn"`, or `"error"`.

### 6.4 Tool approval

Approval is correlated by `request_id`.

#### `approval_request` (A→H, capability `tool_approval`)

```json
{ "type": "approval_request", "request_id": "r1", "call_id": "c1",
  "tool_name": "Bash", "input": { "command": "rm -rf build" } }
```

#### `approval_response` (H→A)

```json
{ "type": "approval_response", "request_id": "r1", "decision": "allow",
  "updated_input": { "command": "rm -rf build" }, "message": null }
```

- `decision` is `"allow"` or `"deny"`.
- `updated_input` (object, optional): an **opaque passthrough**. When present, the adapter uses it in
  place of the original tool input (e.g. the Claude adapter injects `dangerouslyDisableSandbox` here for
  sandbox-bypass rules). When absent, the original input stands.
- `message` (string, optional): a human-readable reason, typically included on denial.

The **host** is the sole authority on approval decisions (via its tool policy). The adapter only asks
and applies the answer.

### 6.5 Errors

#### `error` (A→H)

```json
{ "type": "error", "fatal": true, "code": "spawn_failed", "message": "claude not found on PATH" }
```

`fatal: true` indicates the adapter is exiting. Non-fatal errors are informational.

## 7. Capabilities and graceful degradation

Adapters advertise capability tokens in `ready.capabilities`. The host adapts its behavior when a
capability is absent:

| Capability | Meaning | Host behavior when absent |
| --- | --- | --- |
| `streaming` | Incremental `message` frames during a turn | Host renders only whole messages |
| `thinking` | Emits `thinking` blocks | No reasoning shown |
| `tool_approval` | Supports `approval_request`/`approval_response` | `RequireApproval` policy cannot be honored; policy must resolve to allow/deny before launch; holds disabled |
| `usage_reporting` | Populates `usage` token counts | Usage stats absent (defaults) |
| `cost_reporting` | Populates `usage.total_cost_usd` | Cost absent |
| `context_clear` | Handles `clear_context` | Auto-clear triggers an agent **restart** instead |
| `cancel` | Handles `cancel` | Cancel is best-effort; host may fall back to kill/restart |
| `mcp` | Honors `tools.mcp_servers` | MCP servers not provisioned |
| `system_prompt_append` | Supports `system_prompt.mode = "append"` | Host must send `replace` |
| `resume` | Emits/consumes `resume_token` | Resume unavailable |

Unknown capability tokens MUST be ignored by the host.

## 8. Lifecycle

```
host launches adapter (transport env set)
        │
        ▼
H→A  initialize
        │
A→H  ready                 (host validates protocol_version + capabilities)
        │
   ┌────┴─────────── per turn ───────────────┐
H→A  prompt(turn_id)                          │
A→H  status(busy)                             │
A→H  message* / tool_call*                    │
      (per tool_call, if approval required:)  │
A→H    approval_request(request_id) ──────────┤
H→A    approval_response(request_id)          │
A→H  turn_complete(turn_id, usage?)           │
A→H  status(idle)                             │
   └──────────────────────────────────────────┘
        │
H→A  shutdown            (or clear_context to reset and continue)
        │
        ▼
adapter exits
```

## 9. Conformance checklist

An adapter is AAP v1 compliant if it:

1. Supports the stdio binding (§4.1) and honors `AGENTD_AAP_TRANSPORT`.
2. Accepts `initialize`, refuses unsupported `protocol_version` with a fatal `error`.
3. Emits exactly one `ready` with an accurate `capabilities` list before accepting prompts.
4. Never emits output for a turn before receiving that turn's `prompt`.
5. Echoes the correct `turn_id` on all turn output and closes each turn with `turn_complete`.
6. If it advertises `tool_approval`, emits `approval_request` for gated calls and honors
   `approval_response` (including `updated_input` passthrough) before proceeding.
7. Ignores unknown fields and unknown message types without failing.
8. Handles `shutdown` by terminating its native agent and exiting cleanly.
9. Writes only human-readable logs to stderr (stdio binding).

A **mock adapter** and a **protocol test-vector file** ship with agentd for validating both adapters
and the host.
