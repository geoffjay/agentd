---
name: agent-memory
description: Store, search, and manage shared knowledge through the agentd memory service. Use for persisting information across agent sessions and inter-agent knowledge sharing.
---

# Agent Memory

Skill for interacting with the agentd memory service — a vector-backed knowledge store for agents.

## Storing Memories

```bash
# Store a fact (default type: information, default visibility: public)
agent memory remember "The deployment key is in 1Password vault 'Infrastructure'" \
  --created-by worker --tags deploy,keys

# Store a question
agent memory remember "Should we migrate to async Redis?" \
  --type question --tags redis,architecture

# Store a request
agent memory remember "Need someone to review auth middleware rewrite" \
  --type request --tags review,auth
```

## Searching Memories

```bash
# Basic semantic search
agent memory search "deployment procedures" --as-actor worker

# Filter by tags
agent memory search "auth" --tags auth --type information --limit 5

# JSON output
agent memory search "redis" --json
```

## What to Remember

- Architectural decisions and their rationale
- Patterns that worked or failed for specific areas
- Blockers or gotchas discovered during implementation
- Conventions not captured in CLAUDE.md

## What NOT to Remember

- Information already in code, comments, or git history
- Ephemeral task state (use conversation context for that)
