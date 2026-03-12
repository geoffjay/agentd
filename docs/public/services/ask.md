# agentd-ask API Documentation

The ask service monitors the user's environment and creates interactive notifications that require responses. It integrates with the notification service to track questions and answers.

## Base URL

```
http://127.0.0.1:17001
```

Port defaults to `17001` (dev) or `7001` (production), configurable via the `AGENTD_PORT` environment variable.

## How It Works

1. A client (or cron job) calls `POST /trigger` to run environment checks
2. If a check detects a condition needing attention (e.g., no tmux sessions running), the ask service creates a notification in the notify service with `requires_response: true`
3. The notification appears in the user's notification list
4. The user responds via `POST /answer` with a question ID and answer text
5. The ask service updates the notification in the notify service and processes the answer

Questions have a cooldown period — the same check won't create duplicate notifications within the cooldown window.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17001` | HTTP listen port |
| `AGENTD_NOTIFY_SERVICE_URL` | `http://localhost:7004` | Base URL of the notification service |
| `RUST_LOG` | `info` | Log level filter |

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "service": "agentd-ask",
  "version": "0.2.0",
  "notification_service_url": "http://localhost:17004"
}
```

**Example:**
```bash
curl -s http://127.0.0.1:17001/health | jq
```

---

### Trigger Checks

Run environment checks and create notifications for conditions that need user attention.

```
POST /trigger
```

**Request Body:** None required.

**Response:**
```json
{
  "checks_run": ["tmux_sessions"],
  "notifications_sent": ["550e8400-e29b-41d4-a716-446655440000"],
  "results": {
    "tmux_sessions": {
      "running": false,
      "session_count": 0,
      "sessions": []
    }
  }
}
```

**Response Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `checks_run` | string[] | Names of checks that were executed |
| `notifications_sent` | UUID[] | IDs of notifications created in the notify service |
| `results` | object | Detailed results keyed by check name |

**Check Results — `tmux_sessions`:**

| Field | Type | Description |
|-------|------|-------------|
| `running` | boolean | Whether any tmux sessions are active |
| `session_count` | integer | Number of active sessions |
| `sessions` | string[] | Names of active sessions (omitted if none) |

**Behavior:**

- If tmux sessions **are** running: returns results with an empty `notifications_sent` array
- If tmux sessions **are not** running and cooldown has expired: creates a notification and returns its ID
- If tmux sessions **are not** running but within cooldown: returns results with empty `notifications_sent`

**Errors:**

| Status | Condition |
|--------|-----------|
| 500 | tmux is not installed |
| 500 | Notification service unreachable |

**Examples:**

```bash
# Trigger checks
curl -s -X POST http://127.0.0.1:17001/trigger | jq

# Example: sessions are running (no notification created)
# {
#   "checks_run": ["tmux_sessions"],
#   "notifications_sent": [],
#   "results": {
#     "tmux_sessions": {
#       "running": true,
#       "session_count": 2,
#       "sessions": ["main", "work"]
#     }
#   }
# }
```

---

### Submit Answer

Provide an answer to a pending question.

```
POST /answer
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `question_id` | UUID | yes | ID of the question to answer |
| `answer` | string | yes | The user's response text |

```json
{
  "question_id": "550e8400-e29b-41d4-a716-446655440000",
  "answer": "yes"
}
```

**Response (success):**
```json
{
  "success": true,
  "message": "Answer recorded for question 550e8400-e29b-41d4-a716-446655440000",
  "question_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Question ID not found |
| 410 | Question is no longer pending (already answered or expired) |

**Example:**
```bash
curl -s -X POST http://127.0.0.1:17001/answer \
  -H "Content-Type: application/json" \
  -d '{
    "question_id": "550e8400-e29b-41d4-a716-446655440000",
    "answer": "yes"
  }' | jq
```

---

## Check Types

| Check | Trigger Condition | Question Created |
|-------|-------------------|-----------------|
| `tmux_sessions` | No tmux sessions are running | "Would you like to start a tmux session?" |

More check types are planned for future releases.

## Question Lifecycle

```
POST /trigger (no tmux sessions, cooldown expired)
    │
    ▼
Question created (status: Pending)
Notification created in notify service (requires_response: true)
    │
    ├──▶ User calls POST /answer
    │       │
    │       ▼
    │    Question updated (status: Answered)
    │    Notification updated (status: Responded)
    │
    └──▶ Cooldown timer expires, no answer
            │
            ▼
         Question cleaned up (status: Expired)
```

## Cooldown Mechanism

After creating a notification, the ask service enforces a cooldown period before the same check type can create another notification. This prevents spamming the user with repeated questions.

- Cooldown is tracked per check type
- The cooldown resets when a new notification is successfully created
- If the ask service restarts, cooldown state is lost (in-memory only)

## Running the Service

```bash
# Development
cargo run -p agentd-ask

# With custom notify service URL
AGENTD_NOTIFY_SERVICE_URL=http://localhost:17004 cargo run -p agentd-ask

# With debug logging
RUST_LOG=debug cargo run -p agentd-ask
```
