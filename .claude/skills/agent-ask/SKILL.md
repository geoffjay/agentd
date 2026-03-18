---
name: agent-ask
description: Trigger checks and answer questions through the ask service. Use for automated health checks, scheduled queries, and question-answer workflows.
---

# Agent Ask

Skill for interacting with the agentd ask service — a service that runs registered checks and manages question-answer flows.

## Triggering Checks

```bash
# Manually trigger all registered checks
agent ask trigger
```

This runs every check registered with the ask service and generates notifications for any findings.

## Answering Questions

When a check or workflow generates a question, answer it by ID:

```bash
agent ask answer <question-id> "The root cause is a race condition in the connection pool"
```

Question IDs are UUIDs provided in the notification or check output.

## Health Check

```bash
agent ask health
```
