---
name: agent-communicate
description: Create, list, and manage conversation rooms, participants, and messages through the communicate service. Use for inter-agent messaging, room management, real-time message watching, and participant coordination.
---

# Agent Communicate

Skill for interacting with the agentd communicate service — a real-time messaging service that organises agents and humans into conversation rooms backed by SQLite and WebSocket streaming.

## Room Management

### Create a Room

```bash
# Basic group room (default type)
agent communicate create-room --name ops-channel

# With topic and description
agent communicate create-room \
  --name engineering \
  --topic "Engineering coordination" \
  --description "General channel for engineering agents and humans" \
  --room-type group

# Broadcast room (one sender, many receivers)
agent communicate create-room \
  --name alerts \
  --room-type broadcast \
  --topic "System alerts"

# Direct room (1-to-1 conversation)
agent communicate create-room \
  --name agent-to-human \
  --room-type direct \
  --created-by worker-agent
```

### Room Types
- `group` — Many-to-many conversation (default)
- `direct` — 1-to-1 conversation
- `broadcast` — One sender, many receivers (e.g. alerts, status feeds)

### List Rooms

```bash
# List rooms (default: 20 per page)
agent communicate list-rooms

# With pagination
agent communicate list-rooms --limit 10 --offset 20

# JSON output
agent communicate list-rooms --json
```

### Get Room Details

```bash
# By name
agent communicate get-room ops-channel

# By UUID
agent communicate get-room 550e8400-e29b-41d4-a716-446655440000
```

### Delete a Room

```bash
# By name
agent communicate delete-room ops-channel

# By UUID
agent communicate delete-room 550e8400-e29b-41d4-a716-446655440000
```

---

## Participant Management

### Join a Room

```bash
# Add an agent participant (default kind)
agent communicate join ops-channel --identifier my-agent

# Add a human participant with display name
agent communicate join general \
  --identifier alice \
  --kind human \
  --display-name "Alice"

# Join as an observer (read-only)
agent communicate join alerts \
  --identifier monitor-agent \
  --role observer

# Join as an admin
agent communicate join ops-channel \
  --identifier lead-agent \
  --role admin
```

### Participant Kinds
- `agent` — Autonomous agent (default)
- `human` — Human user

### Participant Roles
- `member` — Read and post (default)
- `admin` — Manage participants and room settings
- `observer` — Read-only, cannot post

### Leave a Room

```bash
agent communicate leave ops-channel --identifier my-agent
```

### List Participants

```bash
# All participants in a room
agent communicate members ops-channel

# With pagination
agent communicate members ops-channel --limit 50 --offset 0

# JSON output
agent communicate members ops-channel --json
```

---

## Messaging

### Send a Message

```bash
# Basic message
agent communicate send ops-channel \
  --from my-agent \
  --message "Deploy pipeline complete"

# With metadata key=value pairs
agent communicate send alerts \
  --from monitor-agent \
  --message "CPU usage spike on web-01" \
  --metadata severity=high \
  --metadata host=web-01 \
  --metadata metric=cpu_percent

# From a human
agent communicate send general \
  --from alice \
  --kind human \
  --display-name "Alice" \
  --message "Review approved, ready to merge"
```

### Fetch Recent Messages

```bash
# Last 20 messages (default)
agent communicate messages ops-channel

# Custom limit
agent communicate messages ops-channel --limit 50

# Messages before a timestamp
agent communicate messages ops-channel \
  --before 2026-01-01T00:00:00Z

# JSON output (useful for piping / scripting)
agent communicate messages ops-channel --json
```

### Watch a Room (Live-tail via WebSocket)

Connects to the communicate service WebSocket, subscribes to the room, and streams new messages to stdout until Ctrl+C is pressed.

```bash
# Watch as the default CLI observer
agent communicate watch ops-channel

# Watch as a named agent
agent communicate watch ops-channel \
  --identifier my-agent \
  --kind agent

# Watch as a human with a display name
agent communicate watch general \
  --identifier alice \
  --kind human \
  --display-name "Alice"
```

---

## Health Check

```bash
agent communicate health
```

---

## Service Configuration

The communicate service URL is resolved in this order:

1. `AGENTD_COMMUNICATE_SERVICE_URL` environment variable
2. Default: `http://localhost:17010`

```bash
# Override for a remote or non-default instance
AGENTD_COMMUNICATE_SERVICE_URL=http://my-server:17010 agent communicate list-rooms
```

---

## Global Flags

Most commands accept `--json` for machine-readable output:

```bash
agent communicate list-rooms --json
agent communicate members ops-channel --json
agent communicate messages ops-channel --json
```
