# Linear Triggers

A `linear_issues` workflow polls the Linear API at a configurable interval and dispatches a task to your agent for every matching issue it has not seen before. Unlike webhook triggers, no public endpoint is required — agentd reaches out to Linear rather than waiting for Linear to push events.

```
agentd                                     Linear
──────────────────────────────────         ──────────────────────
WorkflowRunner timer fires            ──▶  GraphQL API
LinearStrategy fetches matching issues◀──  Issue list response
Deduplicate by identifier (ENG-123)
Dispatch new issues as Tasks          ──▶  Agent processes each issue
```

---

## Prerequisites

### 1. Create a Linear personal API key

1. Open Linear and go to **Settings** → **API** → **Personal API keys**
2. Click **Create key**
3. Give it a descriptive label (e.g. `agentd-worker`)
4. Copy the generated key — it is shown only once

!!! warning "Key scope"
    Personal API keys have the same permissions as your Linear account. Use a dedicated service account or a limited-scope key for production deployments.

### 2. Set the environment variable

agentd reads the Linear API key from the `AGENTD_LINEAR_API_KEY` environment variable. Set it before starting the orchestrator:

```bash
export AGENTD_LINEAR_API_KEY="lin_api_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
```

For persistent configuration, add it to your shell profile or use a secrets manager:

```bash
# .env file (do not commit to version control)
AGENTD_LINEAR_API_KEY=lin_api_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

```bash
# systemd service override
systemctl edit agentd-orchestrator
# Add:
# [Service]
# Environment=AGENTD_LINEAR_API_KEY=lin_api_...
```

If `AGENTD_LINEAR_API_KEY` is not set when a `linear_issues` workflow is created or starts polling, the orchestrator logs an error and the workflow runner exits.

---

## Configuration

### JSON (REST API)

```json
{
  "name": "linear-triage",
  "agent_id": "<AGENT_UUID>",
  "trigger_config": {
    "type": "linear_issues",
    "team_key": "ENG",
    "status": ["Triage"],
    "labels": ["agent"]
  },
  "prompt_template": "Triage Linear issue {{identifier}}: {{title}}\n\n{{body}}\n\nTeam: {{team}} | Priority: {{priority}}",
  "poll_interval_secs": 120,
  "enabled": true
}
```

**Field reference:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"linear_issues"` |
| `team_key` | string | No* | Linear team key (e.g. `ENG`). Filter to issues in this team only |
| `project` | string | No* | Linear project name filter. Issues must belong to this project |
| `status` | array of strings | No* | Issue state names to match (e.g. `["Triage", "Todo"]`) |
| `labels` | array of strings | No* | Label names — issue must carry **all** listed labels |
| `assignee` | string | No* | Assignee display name or email address |

\* At least one filter field must be provided. A config with no filters is rejected to prevent accidentally polling all issues in your workspace.

### CLI

```bash
agent orchestrator create-workflow \
  --name linear-triage \
  --agent-name triage-agent \
  --trigger-type linear-issues \
  --linear-team-key ENG \
  --linear-status "Triage" \
  --linear-label "agent" \
  --poll-interval 120 \
  --prompt-template "Triage Linear issue {{identifier}}: {{title}}\n\n{{body}}"
```

### YAML template (`.agentd/`)

```yaml
name: linear-triage
agent: triage-agent

source:
  type: linear_issues
  team_key: ENG                    # Filter by team key
  status: [Triage]                 # Only issues in "Triage" state
  labels: [agent]                  # Must carry the "agent" label
  # project: Backend               # Optional: filter by project name
  # assignee: alice@example.com    # Optional: filter by assignee

poll_interval_secs: 120
enabled: true

prompt_template: |
  Triage Linear issue {{identifier}}: {{title}}

  {{body}}

  Team: {{team_name}} ({{team}}) | Priority: {{priority}} | State: {{state}}
  URL: {{url}}
```

---

## Filter Options

Filters are applied server-side via the Linear GraphQL API. Only issues matching **all** provided filters are returned.

### `team_key`

The short team identifier shown in issue identifiers (e.g. the `ENG` in `ENG-123`). Find team keys in Linear under **Settings** → **Teams**.

```yaml
team_key: ENG
```

### `project`

The display name of a Linear project. Issues without a project are excluded when this filter is set.

```yaml
project: Backend
```

### `status`

One or more workflow state names. Matches issues in any of the listed states. State names are case-sensitive and must match the names configured in your Linear team's workflow.

```yaml
status: [Triage]
# or multiple states:
status: [Triage, Todo, "In Progress"]
```

Common Linear default states: `Triage`, `Backlog`, `Todo`, `In Progress`, `In Review`, `Done`, `Cancelled`.

### `labels`

One or more label names. An issue must carry **all** listed labels to match (AND logic). Label names are case-sensitive.

```yaml
labels: [bug]
# or multiple labels (issue must have ALL of them):
labels: [bug, agent]
```

### `assignee`

Filter by assignee display name or email address. Leave unset to match issues regardless of assignee (including unassigned).

```yaml
assignee: alice@example.com
```

---

## Template Variables

Linear-specific variables available in `prompt_template`:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{title}}` | Issue title | `"Fix login timeout"` |
| `{{body}}` | Issue description (markdown) | Full markdown content |
| `{{identifier}}` | Linear issue identifier | `ENG-123` |
| `{{state}}` | Issue state name | `Todo`, `In Progress` |
| `{{priority}}` | Priority level | `0` (none), `1` (urgent), `2` (high), `3` (medium), `4` (low) |
| `{{team}}` | Linear team key | `ENG` |
| `{{team_name}}` | Linear team display name | `Engineering` |
| `{{project}}` | Linear project name (empty if unset) | `Backend` |
| `{{assignee}}` | Assignee display name (empty if unassigned) | `Alice Example` |
| `{{labels}}` | Comma-separated label names | `"bug, urgent"` |
| `{{url}}` | Linear issue URL | `https://linear.app/myorg/issue/ENG-123` |
| `{{linear_id}}` | Internal Linear UUID | `abc-uuid-...` |
| `{{source_id}}` | Linear issue identifier (e.g. `ENG-123`) | `ENG-123` |

**Priority values:**

| Value | Meaning |
|-------|---------|
| `0` | No priority |
| `1` | Urgent |
| `2` | High |
| `3` | Medium |
| `4` | Low |

**Example template using Linear variables:**

```
Triage Linear issue {{identifier}}: {{title}}

{{body}}

Team: {{team_name}} ({{team}})
Project: {{project}}
State: {{state}}
Priority: {{priority}}
Assignee: {{assignee}}
Labels: {{labels}}
URL: {{url}}

Please:
1. Review the issue description
2. Assess severity based on priority and labels
3. Suggest next steps or implementation approach
```

---

## Deduplication

Each Linear issue has a human-readable identifier (e.g. `ENG-123`). agentd records processed identifiers so each issue triggers the agent **at most once**, even across restarts. An issue is re-dispatched only if:

- It was never processed (new issue matching the filters)
- The deduplication store is cleared (e.g. database reset)

Linear deduplication uses `issue.identifier` (the same value as `{{source_id}}` in templates). The internal UUID (`{{linear_id}}`) is available in templates for reference but is not used as the deduplication key.

---

## Polling Behaviour

The `poll_interval_secs` field (in seconds, default `60`) controls how often agentd queries the Linear API. At each poll:

1. Linear GraphQL API is queried with the configured filters
2. Each returned issue is checked against the dedup store
3. New issues are converted to `Task` objects and dispatched to the agent
4. Issue identifiers (e.g. `ENG-123`) are recorded to prevent re-dispatch

**Choosing a poll interval:**

| Scenario | Recommended interval |
|----------|---------------------|
| Low-volume team (< 10 issues/day) | `300` (5 min) |
| Active team with real-time needs | `60` (1 min, default) |
| High-frequency triage | `30` (30 s) |

!!! note "Linear API rate limits"
    Linear's API allows up to 1 500 requests per hour per API key. A `poll_interval_secs` of `60` with a single workflow uses ~60 requests/hour, well within limits. If you run many Linear workflows with the same key, increase the interval accordingly.

---

## Webhook Setup (Optional)

For sub-second latency, configure a Linear webhook to push events to agentd instead of polling. This requires a public HTTPS endpoint.

### Step-by-step

**1. Create a webhook workflow in agentd:**

```bash
agent orchestrator create-workflow \
  --name linear-webhook-handler \
  --agent-name worker \
  --trigger-type webhook \
  --webhook-secret "$(openssl rand -hex 32)" \
  --prompt-template "Linear issue {{action}}: {{title}}\n\n{{body}}"
```

**2. Expose the endpoint (development):**

```bash
ngrok http 17006
# → https://abc123.ngrok.io
```

**3. Register the webhook in Linear:**

1. Open Linear → **Settings** → **API** → **Webhooks**
2. Click **New webhook**
3. **URL:** `https://your-domain.com/webhooks/<WORKFLOW_ID>`
4. **Label:** `agentd-worker` (or any descriptive name)
5. **Resource types:** Select `Issues` (and optionally `Comments`, `Projects`)
6. **Team filter:** Optionally restrict to a specific team
7. **Signing secret:** Copy the secret you used with `--webhook-secret` and paste it here
8. Click **Create webhook**

**4. Verify:**

In Linear's webhook settings, click **Send test** and confirm agentd responds with `202 Accepted`.

!!! tip "Polling vs webhooks for Linear"
    Use `linear_issues` polling when you don't need sub-second latency or can't expose a public endpoint. Use the `webhook` trigger type when you need immediate reaction and have a public HTTPS endpoint available.

---

## Troubleshooting

### Workflow not picking up new Linear issues

1. **Check the API key** — verify `AGENTD_LINEAR_API_KEY` is set and valid:
   ```bash
   curl -s -X POST https://api.linear.app/graphql \
     -H "Authorization: $AGENTD_LINEAR_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"query": "{ viewer { id name } }"}' | jq .
   ```
2. **Check filter values** — team keys and state names are case-sensitive. Run:
   ```bash
   agent orchestrator get-workflow <WORKFLOW_ID>
   ```
   and verify `trigger_config` matches your Linear team's actual keys and state names.
3. **Check dispatch history** — `agent orchestrator workflow-history <WORKFLOW_ID>` shows recent dispatches.
4. **Check logs** — the orchestrator logs each Linear poll at `DEBUG` level and each dispatch at `INFO`.

### Issues dispatched multiple times

- This should not happen under normal operation. If it does, check for duplicate workflows polling the same filters.
- Do **not** clear the deduplication store while workflows are running.

### `AGENTD_LINEAR_API_KEY` not found error

The environment variable must be set in the same environment where the orchestrator process runs. If using a process manager (systemd, launchd), set it in the service's environment configuration — not just your shell.

### State name mismatch

Linear state names are workspace-configurable. `"In Progress"` in one workspace may be `"In-Progress"` in another. Find exact names in Linear under **Settings** → **Teams** → your team → **Workflow**.
