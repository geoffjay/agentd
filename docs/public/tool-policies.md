# Tool Policies and Human-in-the-Loop Approvals

Tool policies control which tools an AI agent can use during execution. They provide defense-in-depth for automated agent workflows, ensuring agents operate within defined boundaries.

## Why Tool Policies Matter

Without restrictions, an agent has full access to all Claude Code tools — including `Bash` (arbitrary command execution), `Write` (file modification), and `Edit`. For production workflows, you want to:

- **Limit blast radius** — a code review agent only needs `Read` and `Grep`, not `Bash`
- **Prevent accidental damage** — block `Write` for analysis-only tasks
- **Require human oversight** — hold every tool use for approval during sensitive operations
- **Audit tool usage** — log which tools were requested and the policy decision

## The Five Policy Modes

### AllowAll (default)

No restrictions. The agent can use any tool.

```json
{"mode": "allow_all"}
```

**Use when:** You trust the agent fully, or it's running in an isolated environment.

### DenyAll

Block all tool usage. The agent can only respond with text.

```json
{"mode": "deny_all"}
```

**Use when:** You want the agent to answer questions without performing any actions.

### AllowList

Only the listed tools are permitted. Everything else is denied.

```json
{
  "mode": "allow_list",
  "tools": ["Read", "Grep", "Glob", "WebFetch"]
}
```

**Use when:** You want a read-only agent (code review, analysis, documentation).

### DenyList

All tools are allowed except the listed ones.

```json
{
  "mode": "deny_list",
  "tools": ["Bash", "Write", "Edit"]
}
```

**Use when:** You want most capabilities but need to block specific dangerous tools.

### RequireApproval

Every tool request is held pending until a human approves or denies it. The agent blocks (waits) on each tool call while you decide.

```json
{"mode": "require_approval"}
```

**Use when:** Maximum oversight is needed — security audits, untrusted tasks, learning what an agent does before granting broader access.

## Setting Policies

### On agent creation

```bash
# CLI
agent orchestrator create-agent \
  --name safe-reviewer \
  --tool-policy '{"mode":"allow_list","tools":["Read","Grep","Glob"]}'

# API
curl -X POST http://localhost:17006/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "safe-reviewer",
    "working_dir": "/path/to/project",
    "tool_policy": {"mode": "allow_list", "tools": ["Read", "Grep"]}
  }'
```

### On a running agent

```bash
# CLI
agent orchestrator set-policy <AGENT_ID> '{"mode":"deny_list","tools":["Bash"]}'
agent orchestrator get-policy <AGENT_ID>

# API
curl -X PUT http://localhost:17006/agents/<ID>/policy \
  -H "Content-Type: application/json" \
  -d '{"mode": "deny_list", "tools": ["Bash", "Write"]}'
```

### In YAML templates

```yaml
# .agentd/agents/reviewer.yml
name: reviewer
working_dir: "."
tool_policy:
  mode: allow_list
  tools:
    - Read
    - Grep
    - Glob
```

### On workflows

Workflows can set a tool policy that is applied to the agent before each task dispatch:

```yaml
# .agentd/workflows/safe-review.yml
name: safe-review
agent: reviewer
tool_policy:
  mode: allow_list
  tools: [Read, Grep, Glob]
source:
  type: github_issues
  owner: myorg
  repo: myrepo
prompt_template: "Review: {{title}}\n{{body}}"
```

## Human-in-the-Loop Approvals

When an agent runs with `require_approval` policy, the orchestrator holds each tool request until a human decides.

### How it works

1. Agent sends a `control_request` (e.g., "can I use `Read` on `src/main.rs`?")
2. Orchestrator detects `RequireApproval` policy
3. Request is stored in the `ApprovalRegistry` with a unique ID
4. A `pending_approval` event is broadcast on the `/stream` WebSocket
5. The WebSocket response is **held** — the agent blocks waiting
6. A human calls `approve` or `deny` via CLI or API
7. The decision is sent to the agent, which proceeds or adapts
8. If no decision within **5 minutes**, the request is auto-denied

### Managing approvals

```bash
# List pending approvals
agent orchestrator list-approvals
agent orchestrator list-approvals --agent-id <ID>

# Approve a tool request
agent orchestrator approve <APPROVAL_ID>

# Deny a tool request
agent orchestrator deny <APPROVAL_ID>

# JSON output for scripting
agent orchestrator list-approvals --json
```

### Monitoring via stream

Pending approval events appear in the real-time stream:

```bash
agent orchestrator stream --all
```

Output includes:
```
[abc12345] ⚡ Permission request: Read
```

### API endpoints

```
GET  /approvals                    # list all (filter: ?status=pending)
GET  /approvals/{id}               # get single approval
POST /approvals/{id}/approve       # approve
POST /approvals/{id}/deny          # deny
GET  /agents/{id}/approvals        # per-agent approvals
```

### Timeout behavior

Unanswered approvals are automatically denied after 5 minutes. The agent receives a deny response with the message: `"approval timeout — no human decision within 5 minutes"`.

## Recommended Policies by Use Case

| Use Case | Policy | Tools |
|----------|--------|-------|
| Code review | `allow_list` | Read, Grep, Glob |
| Documentation | `allow_list` | Read, Grep, Glob, WebFetch |
| Bug fixing | `allow_all` | (all) |
| Security audit | `require_approval` | (human decides each) |
| CI/CD agent | `deny_list` | Block: Bash, Write |
| Learning/testing | `require_approval` | (observe what agent requests) |

## Audit Logging

All tool policy decisions are logged with structured fields:

```
INFO agent_id=550e8400 tool_name=Read decision=allow policy_mode=allow_list
WARN agent_id=550e8400 tool_name=Bash decision=deny policy_mode=deny_list
```

Log fields include:
- `agent_id` — which agent
- `tool_name` — which tool was requested
- `decision` — `allow` or `deny`
- `policy_mode` — which policy mode drove the decision
