---
name: agent-memory
description: Store, search, and manage shared knowledge through the memory service. Use for persisting information across agent sessions, semantic search over stored knowledge, and inter-agent knowledge sharing.
---

# Agent Memory

Skill for interacting with the agentd memory service — a vector-backed knowledge store for agents.

## Storing Memories

```bash
# Store a simple fact
agent memory remember "The deployment key is stored in 1Password vault 'Infrastructure'"

# With metadata
agent memory remember "OAuth tokens expire after 24 hours" \
  --created-by worker-agent \
  --type information \
  --tags auth,tokens,expiry

# Store a question for later resolution
agent memory remember "Should we migrate to async Redis client?" \
  --type question \
  --tags redis,architecture

# Store a request
agent memory remember "Need someone to review the auth middleware rewrite" \
  --type request \
  --tags review,auth

# Control visibility
agent memory remember "API key rotation schedule: first Monday of each month" \
  --visibility shared \
  --share-with ops-agent,deploy-agent

# Private memory (only visible to creator)
agent memory remember "My working hypothesis: the leak is in the connection pool" \
  --visibility private \
  --created-by debug-agent

# Reference other memories
agent memory remember "Follow-up: the Redis migration decision was made" \
  --references <memory-id-1>,<memory-id-2>
```

### Memory Types
- `information` — facts, decisions, observations (default)
- `question` — open questions needing resolution
- `request` — action items or asks

### Visibility Levels
- `public` — visible to all agents (default)
- `shared` — visible to creator and specified actors
- `private` — visible only to creator

## Searching Memories

Semantic similarity search over all stored memories:

```bash
# Basic search
agent memory search "how do we handle token expiry"

# Search as a specific actor (respects visibility)
agent memory search "deployment procedures" --as-actor deploy-agent

# Filter by type and tags
agent memory search "auth" --type information --tags auth

# Limit results
agent memory search "Redis" --limit 5

# Time-bounded search
agent memory search "incidents" --since 2026-01-01T00:00:00Z --until 2026-03-01T00:00:00Z

# JSON output
agent memory search "auth" --json
```

## Retrieving and Listing

```bash
# Get a specific memory by ID
agent memory recall <memory-id>

# List memories with filters
agent memory list
agent memory list --type question
agent memory list --tag auth
agent memory list --created-by worker-agent
agent memory list --visibility public
agent memory list --limit 20 --offset 40
```

## Updating Visibility

```bash
# Make a memory shared with specific agents
agent memory visibility <memory-id> shared --share-with ops-agent,deploy-agent

# Make public
agent memory visibility <memory-id> public

# Make private
agent memory visibility <memory-id> private
```

## Deleting Memories

```bash
agent memory forget <memory-id>
```

## Health Check

```bash
agent memory health
```
