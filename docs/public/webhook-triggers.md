# Webhook Triggers

A webhook workflow fires every time an HTTP `POST` arrives at its dedicated endpoint. Unlike schedule triggers (which run on a timer) or event triggers (which react to internal events), webhooks are driven by external systems — GitHub, CI pipelines, monitoring tools, or any HTTP client.

```
External system                          agentd
──────────────                           ──────────────────────────────────────
GitHub / CI / curl ──POST /webhooks/──▶  Verify HMAC signature
                      {workflow_id}       Parse payload into Task
                                          Push task to WebhookStrategy
                                          WorkflowRunner dispatches to agent
```

---

## Configuration

### JSON (REST API)

```json
{
  "name": "github-issue-handler",
  "agent_id": "<AGENT_UUID>",
  "trigger_config": {
    "type": "webhook",
    "secret": "my-hmac-secret"
  },
  "prompt_template": "New GitHub event: {{title}}\n\n{{body}}\n\nURL: {{url}}",
  "enabled": true
}
```

**Field reference:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | Yes | Must be `"webhook"` |
| `secret` | string | No | HMAC-SHA256 signing secret. If omitted, all incoming requests are accepted without verification |

### CLI

```bash
agent orchestrator create-workflow \
  --name github-issue-handler \
  --agent-name worker \
  --trigger-type webhook \
  --webhook-secret "my-hmac-secret" \
  --prompt-template "New GitHub event: {{title}}\n\n{{body}}\n\nURL: {{url}}"
```

`--webhook-secret` is optional. Omit it to disable signature verification.

### YAML template (`.agentd/`)

```yaml
name: github-issue-handler
agent: worker

source:
  type: webhook
  secret: "my-hmac-secret"   # optional

prompt_template: |
  New GitHub event: {{title}}

  {{body}}

  URL: {{url}}
  Action: {{action}}
  Event: {{github_event}}

enabled: true
```

---

## Webhook Endpoint

Once a webhook workflow is created and enabled, it listens for inbound HTTP requests at:

```
POST /webhooks/{workflow_id}
```

Where `{workflow_id}` is the UUID returned when the workflow was created.

### Request

| Component | Value |
|-----------|-------|
| Method | `POST` |
| Content-Type | `application/json` (recommended) or any body |
| `X-Hub-Signature-256` | `sha256=<hex>` — required only when a `secret` is configured |
| `X-GitHub-Event` | Optional — triggers GitHub-specific payload parsing |
| `X-GitHub-Delivery` | Optional — used as the deduplication `delivery_id` |

### Response codes

| Code | Meaning |
|------|---------|
| `202 Accepted` | Payload received and queued for dispatch |
| `401 Unauthorized` | Signature verification failed (wrong secret or missing header) |
| `404 Not Found` | Workflow not found or not currently running |
| `422 Unprocessable Entity` | Workflow exists but is not a webhook trigger type |
| `503 Service Unavailable` | Webhook channel full — the runner cannot keep up |

!!! tip "404 vs 422"
    A `404` means the workflow UUID doesn't exist **or** the workflow runner is not started (e.g. `enabled: false`). A `422` means the workflow exists and is running, but it uses a different trigger type.

---

## HMAC-SHA256 Signature Verification

When a `secret` is configured, the orchestrator verifies every inbound request using HMAC-SHA256 — the same scheme used by GitHub, Slack, and most major webhook providers.

### Verification flow

```
1. Read X-Hub-Signature-256 header
2. Strip "sha256=" prefix → raw hex string
3. Hex-decode to bytes
4. HMAC-SHA256(secret, request_body) → computed bytes
5. Constant-time compare: computed == expected
6. If mismatch → 401 Unauthorized
```

The comparison uses `hmac::Mac::verify_slice`, which is timing-attack-safe. **Source:** `crates/orchestrator/src/scheduler/webhook.rs`

### Header format

The `X-Hub-Signature-256` header must be in GitHub's format:

```
X-Hub-Signature-256: sha256=b5c7be9e6d7c...
```

The `sha256=` prefix is stripped automatically. A raw hex string without the prefix is also accepted.

### When no secret is configured

If `secret` is `null` or omitted from the trigger config, **all inbound requests are accepted without any verification**. Use this only for internal or trusted networks.

!!! warning "Production deployments"
    Always configure a secret for any webhook endpoint reachable from the public internet. Without a secret, any caller that discovers your webhook URL can trigger your workflows.

### Generating a secure secret

```bash
# Generate a random 32-byte secret (hex-encoded)
openssl rand -hex 32
```

Store the secret in your external system (e.g. GitHub webhook settings) and pass the same value as `--webhook-secret` when creating the workflow.

---

## Payload Parsing

The orchestrator automatically detects GitHub webhook payloads based on the presence of the `X-GitHub-Event` header. Non-GitHub payloads fall back to generic parsing.

### GitHub events

When `X-GitHub-Event` is present, the payload is parsed as a structured GitHub event:

#### `issues` events

| GitHub payload field | Task field |
|---------------------|------------|
| `issue.title` | `title` |
| `issue.body` | `body` |
| `issue.html_url` | `url` |
| `issue.labels[].name` | `labels` |
| `issue.assignee.login` | `assignee` |
| `action` | `metadata["action"]` |
| `issue.number` | `metadata["issue_number"]` |

#### `pull_request` events

| GitHub payload field | Task field |
|---------------------|------------|
| `pull_request.title` | `title` |
| `pull_request.body` | `body` |
| `pull_request.html_url` | `url` |
| `pull_request.labels[].name` | `labels` |
| `pull_request.assignee.login` | `assignee` |
| `action` | `metadata["action"]` |
| `pull_request.number` | `metadata["pr_number"]` |

#### Other GitHub events (`push`, `create`, `release`, etc.)

For unrecognised GitHub event types:

- `title` → `"GitHub event: {event_type}"` (e.g. `"GitHub event: push"`)
- `body` → raw JSON string of the full payload
- `url`, `labels`, `assignee` → empty

All GitHub events populate `metadata["github_event"]` with the event type string.

### Generic (non-GitHub) payloads

When `X-GitHub-Event` is absent, the orchestrator parses the body as generic JSON:

- `title` → first non-empty value of `payload.title`, `payload.subject`, or `payload.name`; falls back to `"Webhook payload"`
- `body` → the raw request body (UTF-8 string)
- `url`, `labels`, `assignee` → empty

Non-JSON bodies are accepted — the raw bytes are stored as the `body` string.

### `source_id` and deduplication

Every webhook task gets a unique `source_id`:

```
webhook:{delivery_id}:{timestamp}
```

- `delivery_id` comes from the `X-GitHub-Delivery` header if present; otherwise a random UUID is generated.
- `timestamp` is the RFC 3339 time the request was received.

Because both components vary, each delivery produces a unique `source_id` — duplicate deliveries (GitHub retries on timeout) are dispatched again.

### Metadata fields

All webhook tasks carry these metadata keys (usable as `{{placeholders}}` in templates):

| Key | Description | Always present? |
|-----|-------------|----------------|
| `delivery_id` | Request delivery identifier | Yes |
| `timestamp` | RFC 3339 receive time | Yes |
| `github_event` | GitHub event type (e.g. `issues`) | GitHub requests only |
| `action` | GitHub action (e.g. `opened`, `labeled`) | `issues` / `pull_request` only |
| `issue_number` | GitHub issue number | `issues` events only |
| `pr_number` | GitHub PR number | `pull_request` events only |

---

## Template Variables

Webhook tasks support all standard task variables plus the metadata fields above:

**Standard fields** (available for all trigger types):

| Variable | Description |
|----------|-------------|
| `{{title}}` | Parsed or fallback title |
| `{{body}}` | Issue/PR body or raw payload |
| `{{url}}` | GitHub HTML URL (empty for generic payloads) |
| `{{labels}}` | Comma-separated label names |
| `{{assignee}}` | Assignee login (empty if none) |
| `{{source_id}}` | `webhook:{delivery_id}:{timestamp}` |

**Webhook metadata variables:**

| Variable | Description |
|----------|-------------|
| `{{delivery_id}}` | Delivery identifier from `X-GitHub-Delivery` or auto-generated UUID |
| `{{timestamp}}` | RFC 3339 time the webhook was received |
| `{{github_event}}` | GitHub event type (e.g. `issues`, `pull_request`) |
| `{{action}}` | GitHub action (e.g. `opened`, `labeled`, `closed`) |
| `{{issue_number}}` | Issue number for `issues` events |
| `{{pr_number}}` | PR number for `pull_request` events |

**Example template for GitHub issue events:**

```
GitHub issue event received ({{action}}):
Issue #{{issue_number}}: {{title}}
URL: {{url}}
Labels: {{labels}}
Assigned to: {{assignee}}

Description:
{{body}}

Delivery: {{delivery_id}} at {{timestamp}}
```

---

## GitHub Webhook Setup

### Step-by-step

**1. Create the workflow in agentd**

```bash
# Note the workflow ID in the output
agent orchestrator create-workflow \
  --name github-issue-handler \
  --agent-name worker \
  --trigger-type webhook \
  --webhook-secret "$(openssl rand -hex 32)" \
  --prompt-template "GitHub issue {{action}}: #{{issue_number}} {{title}}\n\n{{body}}\n\nURL: {{url}}"
```

**2. Make the endpoint publicly reachable**

agentd binds to `127.0.0.1` by default. For GitHub to reach it you need a public URL. Options:

=== "Local development (tunnel)"

    ```bash
    # Using ngrok
    ngrok http 17006

    # Or using cloudflared
    cloudflared tunnel --url http://localhost:17006
    ```

    Use the HTTPS URL from the tunnel output.

=== "Production"

    Deploy agentd behind a reverse proxy (nginx, Caddy) with a public domain and TLS.

**3. Configure the GitHub webhook**

1. Go to your repository → **Settings** → **Webhooks** → **Add webhook**
2. **Payload URL:** `https://your-domain.com/webhooks/<WORKFLOW_ID>`
3. **Content type:** `application/json`
4. **Secret:** The same value you used for `--webhook-secret`
5. **SSL verification:** Enable (required for HMAC to be meaningful)
6. **Events:** Select which events to subscribe to (see below)
7. Click **Add webhook**

**4. Recommended GitHub events to subscribe to**

| GitHub event | Use case |
|-------------|---------|
| `Issues` | React to issue creation, labeling, assignment |
| `Pull requests` | React to PR opened, reviewed, merged |
| `Push` | React to code pushes (raw JSON body) |
| `Releases` | React to new releases |

For a focused agent, subscribe only to the events your prompt template expects. Unrecognised events still produce a task but with minimal structured fields.

**5. Verify the webhook fires**

In the GitHub webhook settings page, click **Recent Deliveries** to see the last requests and their response codes. A `202` confirms agentd received and accepted the payload.

---

## Local Testing with curl

You can send test webhooks directly with `curl` during development.

### Unsigned request (no secret configured)

```bash
curl -s -X POST http://127.0.0.1:17006/webhooks/<WORKFLOW_ID> \
  -H "Content-Type: application/json" \
  -d '{"title": "Test task", "body": "This is a test webhook payload"}'
```

Expected response: `202 Accepted` (empty body)

### Simulated GitHub issues event

```bash
curl -s -X POST http://127.0.0.1:17006/webhooks/<WORKFLOW_ID> \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: issues" \
  -H "X-GitHub-Delivery: test-delivery-001" \
  -d '{
    "action": "opened",
    "issue": {
      "number": 42,
      "title": "Fix the login bug",
      "body": "Users cannot log in with SSO.",
      "html_url": "https://github.com/myorg/myrepo/issues/42",
      "labels": [{"name": "bug"}, {"name": "urgent"}],
      "assignee": {"login": "alice"}
    }
  }'
```

### Signed request (with HMAC-SHA256)

```bash
SECRET="my-hmac-secret"
BODY='{"title":"Signed test","body":"This payload is signed"}'
SIG=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$SECRET" -hex | sed 's/SHA2-256(stdin)= //' | sed 's/.* //')

curl -s -X POST http://127.0.0.1:17006/webhooks/<WORKFLOW_ID> \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=${SIG}" \
  -d "$BODY"
```

### Inspect dispatch history after delivery

```bash
agent orchestrator dispatch-history <WORKFLOW_ID>
```

---

## End-to-End Example: GitHub Issues → Agent

This example creates a complete setup where an agent processes any newly opened GitHub issue.

**1. Create the agent:**

```bash
agent orchestrator create-agent --name issue-worker
# → agent ID: <AGENT_UUID>
```

**2. Create the webhook workflow:**

```bash
WEBHOOK_SECRET="$(openssl rand -hex 32)"
echo "Secret: $WEBHOOK_SECRET"   # save this for GitHub

agent orchestrator create-workflow \
  --name github-issues-webhook \
  --agent-name issue-worker \
  --trigger-type webhook \
  --webhook-secret "$WEBHOOK_SECRET" \
  --prompt-template "$(cat <<'TMPL'
A GitHub issue was {{action}} on this repository.

Issue #{{issue_number}}: {{title}}
URL: {{url}}
Labels: {{labels}}
Assigned to: {{assignee}}

Description:
{{body}}

Please:
1. Read the issue carefully
2. Create a branch: issue-{{issue_number}}
3. Implement the required changes
4. Run tests
5. Open a PR that closes this issue
TMPL
)"
# → workflow ID: <WORKFLOW_ID>
```

**3. Expose the endpoint (development):**

```bash
ngrok http 17006
# → Forwarding https://abc123.ngrok.io -> http://localhost:17006
```

**4. Register the webhook in GitHub:**

Payload URL: `https://abc123.ngrok.io/webhooks/<WORKFLOW_ID>`
Secret: the value from `$WEBHOOK_SECRET`
Events: `Issues`

**5. Open a GitHub issue → watch the agent work:**

```bash
# Watch dispatch history update in real time
watch -n 5 'agent orchestrator dispatch-history <WORKFLOW_ID>'

# Stream agent output
agent orchestrator get-agent <AGENT_UUID>
```

---

## Operational Notes

### Webhook vs polling

| Aspect | Webhook | Polling (github_issues) |
|--------|---------|------------------------|
| Latency | Sub-second | Up to `poll_interval_secs` |
| Requires public endpoint | Yes | No |
| Event filtering | All subscribed events | Label/state filters |
| Missed events | If endpoint is down | Catches up on next poll |
| GitHub deduplication | Per delivery ID | Per issue number |

Webhooks are best when you need low latency and can maintain a public endpoint. Polling is more reliable if your network is unstable or you're behind a firewall.

### Channel backpressure

The webhook channel has a capacity of **64 tasks**. If 64 payloads arrive before the agent finishes processing the first, subsequent requests return `503 Service Unavailable`.

The sender (GitHub or other system) is responsible for retry. GitHub automatically retries failed webhook deliveries with exponential backoff over several hours.

To reduce the risk of backpressure:
- Subscribe only to the specific GitHub events your agent needs.
- Configure label filters at the GitHub webhook level (webhook filtering is not available in the agentd trigger config).
- Use a fast agent that doesn't hold the busy slot for long.

### Agent busy or disconnected

The runner dispatches one task at a time. If the agent is busy when a webhook arrives, the task enters the channel queue (up to capacity 64). If the agent is disconnected, the task is queued and dispatched when the agent reconnects.

Dispatch skips are logged:

```
DEBUG orchestrator::scheduler::runner workflow_id=... "Agent busy, skipping dispatch"
DEBUG orchestrator::scheduler::runner workflow_id=... "Agent not connected, skipping dispatch"
```

Note: "skipping dispatch" here means the runner will try again on the next `next_tasks()` call — the task remains in the channel and is not dropped.

### Observing webhook activity in logs

```
INFO  orchestrator::scheduler::api  workflow_id=... source_id=webhook:delivery-001:... title="Fix login bug" "Webhook payload received"
INFO  orchestrator::scheduler::runner workflow_id=... source_id=webhook:... "Dispatched task to agent"
WARN  orchestrator::scheduler::api  "Webhook channel full — workflow runner cannot keep up"
```

### Networking considerations

- agentd binds to `127.0.0.1` — for production, place it behind a reverse proxy that terminates TLS.
- Always use HTTPS for webhook endpoints in production. Without TLS, the payload body (and HMAC signature) are visible to network observers.
- Validate that your reverse proxy forwards the raw (unmodified) request body. HMAC verification fails if the body is transformed (e.g. by re-encoding JSON with different whitespace).
