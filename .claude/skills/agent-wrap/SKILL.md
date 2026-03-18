---
name: agent-wrap
description: Launch, list, and manage AI agents in tmux sessions through the wrap service. Use for running interactive agent sessions with various providers and models.
---

# Agent Wrap

Skill for interacting with the agentd wrap service — manages AI agent sessions running in tmux.

## Launching Agent Sessions

```bash
# Launch Claude Code in a tmux session
agent wrap launch my-session \
  --path /path/to/project \
  --agent claude-code

# Launch with a specific model
agent wrap launch my-session \
  --path . \
  --agent claude-code \
  --model claude-sonnet-4-6

# Launch with a different provider
agent wrap launch my-session \
  --path . \
  --agent crush \
  --provider openai \
  --model gpt-4o

# Launch with Ollama (local models)
agent wrap launch my-session \
  --path . \
  --agent opencode \
  --provider ollama \
  --model llama3

# Launch with custom layout
agent wrap launch my-session \
  --path . \
  --agent claude-code \
  --layout-json '{"width": 200, "height": 50}'
```

### Supported Agents
- `claude-code` — Anthropic's Claude Code CLI
- `crush` — Alternative agent interface
- `opencode` — Open-source code agent

### Supported Providers
- `anthropic` — Anthropic API (default)
- `openai` — OpenAI API
- `ollama` — Local Ollama instance

## Listing Sessions

```bash
# List all active tmux sessions
agent wrap list

# JSON output
agent wrap list --json
```

## Killing Sessions

```bash
agent wrap kill my-session
```

## Health Check

```bash
agent wrap health
```
