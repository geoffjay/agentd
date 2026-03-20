# Communication Guide

This guide shows how to set up the communicate service, create rooms, add participants, and configure agents for collaborative communication.

For the API reference, see [agentd-communicate API](services/communicate.md). For conceptual background, see [Inter-Agent Communication](communication.md).

## Start the Service

=== "Development"
    ```bash
    cargo run -p agentd-communicate
    # Service starts on http://127.0.0.1:17010
    ```

=== "Installed"
    ```bash
    # macOS
    launchctl load ~/Library/LaunchAgents/com.geoffjay.agentd-communicate.plist

    # Linux
    systemctl --user start agentd-communicate.service
    ```

Check that it is running:

```bash
agent communicate health
```

---

## Create a Room

```bash
# Create a group room (the default type)
agent communicate create-room --name engineering \
  --topic "Engineering team coordination" \
  --created-by alice

# Create a broadcast room (admin-only posting)
agent communicate create-room --name announcements \
  --room-type broadcast \
  --created-by alice

# Create a direct (one-to-one) room
agent communicate create-room --name alice-worker \
  --room-type direct \
  --created-by alice
```

!!! note "Room names are permanent"
    A room's name and type cannot be changed after creation. Topic and description can be updated via the REST API.

---

## Add Participants

A participant must be added to a room before they can send messages or subscribe to events.

```bash
# Add an agent as a member (default role)
agent communicate join engineering \
  --identifier worker-agent-uuid \
  --kind agent \
  --display-name "Worker"

# Add a human as an admin
agent communicate join engineering \
  --identifier alice \
  --kind human \
  --display-name "Alice" \
  --role admin

# Add an observer (read-only)
agent communicate join engineering \
  --identifier monitor-agent \
  --kind agent \
  --display-name "Monitor" \
  --role observer
```

List the current members:

```bash
agent communicate members engineering
```

Remove a participant:

```bash
agent communicate leave engineering --identifier worker-agent-uuid
```

---

## Send and Read Messages

```bash
# Send a message as an agent
agent communicate send engineering \
  --from worker-agent-uuid \
  --message "PR #42 is ready for review"

# Send with metadata
agent communicate send engineering \
  --from worker-agent-uuid \
  --message "CPU spike detected" \
  --metadata severity=high \
  --metadata host=web-01

# Read the 20 most recent messages
agent communicate messages engineering

# Read more
agent communicate messages engineering --limit 50

# Read messages before a timestamp
agent communicate messages engineering \
  --before 2026-03-19T12:00:00Z
```

---

## Monitor a Room in Real Time

The `watch` command connects to the communicate service WebSocket and streams new messages to your terminal until you press Ctrl+C.

```bash
# Watch as a human observer (default)
agent communicate watch engineering

# Watch as a specific participant
agent communicate watch engineering \
  --identifier alice \
  --kind human \
  --display-name "Alice"

# JSON output (pipe-friendly)
agent communicate watch engineering --json
```

!!! warning "Membership required for watch"
    The `watch` command uses the WebSocket `subscribe` message, which requires the connecting identifier to be a participant in the room. Add yourself first with `agent communicate join`, or use the default `cli-observer` identifier (which must also be a participant).

---

## Configure Agents with Room Membership via Templates

The most repeatable way to configure rooms is with `.agentd/` templates. Rooms are created before agents during `agent apply`, so agents can reference rooms by name.

### Room Template

Create `.agentd/rooms/engineering.yml`:

```yaml
name: engineering
topic: "Engineering team coordination"
description: "General channel for engineering agents and humans"
type: group
participants:
  - identifier: alice
    kind: human
    role: admin
    display_name: "Alice"
  - identifier: monitor
    kind: agent
    role: observer
    display_name: "Monitor Agent"
```

This creates the room and adds the listed participants when `agent apply` runs. If the room already exists, it is not re-created or modified.

### Agent Template with Room Membership

Add a `rooms` field to `.agentd/agents/worker.yml`:

```yaml
name: worker
working_dir: "."

# Rooms this agent automatically joins when it connects.
rooms:
  - engineering                # plain name — defaults to member role
  - name: announcements
    role: observer             # join as observer
```

The orchestrator adds the agent as a participant in each listed room when the agent starts. If the room does not exist, the apply step will have created it from the corresponding `.agentd/rooms/` template.

### Apply Order

`agent apply .agentd/` processes resources in this order:

1. **Rooms** — `.agentd/rooms/*.yml` created first
2. **Agents** — `.agentd/agents/*.yml` started next (can reference rooms by name)
3. **Workflows** — `.agentd/workflows/*.yml` created last (reference agents by name)

Teardown reverses the order: workflows → agents → rooms.

```bash
# Apply everything
agent apply .agentd/

# Dry run to preview
agent apply --dry-run .agentd/

# Tear down everything (rooms last)
agent teardown .agentd/
```

---

## Example: Two Agents Collaborating via a Shared Room

This example sets up a planner agent and a worker agent that communicate via a shared `engineering` room.

### Directory structure

```
.agentd/
├── rooms/
│   └── engineering.yml
├── agents/
│   ├── planner.yml
│   └── worker.yml
└── workflows/
    └── issue-planner.yml
```

### `.agentd/rooms/engineering.yml`

```yaml
name: engineering
topic: "Planning and execution coordination"
type: group
participants:
  - identifier: alice
    kind: human
    role: admin
    display_name: "Alice"
```

### `.agentd/agents/planner.yml`

```yaml
name: planner
working_dir: "."
rooms:
  - engineering

system_prompt: |
  You are a planning agent. When you receive a new GitHub issue:
  1. Analyse the requirements
  2. Break the work into subtasks
  3. Post your plan to the engineering room:
     agent communicate send engineering --from planner --message "Plan for #{{source_id}}: ..."
  4. Wait for the worker to confirm before finalising
```

### `.agentd/agents/worker.yml`

```yaml
name: worker
working_dir: "."
rooms:
  - engineering

system_prompt: |
  You are a worker agent. Watch the engineering room for plans from the planner:
    agent communicate watch engineering --identifier worker --kind agent
  When you see a plan, implement it, then post a status update to the room.
```

### Launch

```bash
agent apply .agentd/
```

### Monitor the conversation

```bash
# Watch the room as Alice (after adding yourself as a participant via the template)
agent communicate watch engineering \
  --identifier alice \
  --kind human \
  --display-name "Alice"
```

---

## Using the CLI Reference

```bash
# All communicate subcommands
agent communicate --help

# Individual command help
agent communicate create-room --help
agent communicate watch --help
```

### Quick reference

| Task | Command |
|------|---------|
| Create a room | `agent communicate create-room --name NAME` |
| List rooms | `agent communicate list-rooms` |
| Get room details | `agent communicate get-room NAME` |
| Delete a room | `agent communicate delete-room NAME` |
| Add participant | `agent communicate join ROOM --identifier ID --kind KIND` |
| Remove participant | `agent communicate leave ROOM --identifier ID` |
| List members | `agent communicate members ROOM` |
| Send a message | `agent communicate send ROOM --from ID --message TEXT` |
| Read messages | `agent communicate messages ROOM` |
| Live-tail room | `agent communicate watch ROOM` |
