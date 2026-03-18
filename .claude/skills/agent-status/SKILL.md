---
name: agent-status
description: Check the health and availability of all agentd services. Use to verify services are running before performing operations.
---

# Agent Status

Skill for checking the health of agentd services.

## Checking All Services

```bash
# Check health of all services at once
agent status

# JSON output
agent status --json
```

Displays a summary table with each service's status and details.

## Individual Service Health

Each service subcommand also has a `health` check:

```bash
agent orchestrator health
agent notify health
agent ask health
agent wrap health
agent memory health
```

## Service Ports

| Service      | Dev Port | Prod Port | Env Var                        |
|--------------|----------|-----------|--------------------------------|
| Ask          | 17001    | 7001      | `AGENTD_ASK_SERVICE_URL`       |
| Notify       | 17004    | 7004      | `AGENTD_NOTIFY_SERVICE_URL`    |
| Wrap         | 17005    | 7005      | `AGENTD_WRAP_SERVICE_URL`      |
| Orchestrator | 17006    | 7006      | `AGENTD_ORCHESTRATOR_SERVICE_URL` |
| Memory       | 17008    | 7008      | `AGENTD_MEMORY_SERVICE_URL`    |

Default URLs use `http://localhost:<prod-port>`. Override with env vars for non-default setups.
