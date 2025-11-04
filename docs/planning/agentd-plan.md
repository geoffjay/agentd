# Agentd - Services to Assist Human/Agent Workflows

Terminal only agent/LLM tools is the focus of this project. All services should follow the Unix philosophy and do one thing well.

## Scope

- **Single-user**: Designed for one user per machine
- **Terminal-focused**: All interactions via CLI and terminal-based agents
- **Cross-platform**: Linux, macOS, Windows support

## Target Agent Integrations

Initial support for:

- **Claude Code**: Anthropic's official CLI agent
- **opencode**: Open-source coding agent
- **Gemini CLI**: Google's Gemini command-line interface

## Service Implementation Priority

1. **agentd-notify** - Notification service (foundation)
2. **agentd-ask** - Project orchestration daemon
3. **agentd-wrap** - Agent launcher/wrapper
4. **agentd-hook** - GitHub event handling
5. **agentd-monitor** - Sub-agent monitoring

## Services

### Notification Service (agentd-notify)

Manages user notifications and collects responses for the agentd ecosystem.

- JSON HTTP API for other services
- System notifications via notify-rust
- Future: Custom menu bar application for richer interactions
- Stores notification history and responses
- Handles notification expiration and cleanup

See [agentd-notify-plan.md](./agentd-notify-plan.md) for detailed design.

### Ask Service (agentd-ask)

Orchestrates AI agent workflows through human-in-the-loop interactions.

- Maintains registry of projects and locations
- Periodically polls project states
- Sends notifications via agentd-notify to request user input
- Manages tmux sessions for running agents
- Tracks active agent sessions and lifecycle
- Supports local (Ollama) and cloud models (OpenAI, Anthropic, Google)
- Uses agentd-wrap to launch agent CLIs

See [agentd-ask-plan.md](./agentd-ask-plan.md) for detailed design.

### Agent Execution Wrapper (agentd-wrap)

Wraps agent CLI execution with monitoring and lifecycle management.

**Responsibilities:**

- Launch agents with proper configuration
- Monitor agent health and status
- Capture agent output (optional)
- Report exit codes
- Handle agent startup failures

**Interface:**

```bash
agentd-wrap launch \
  --agent-type claude-code \
  --model-provider anthropic \
  --model-name claude-sonnet-4.5 \
  --project-path /path/to/project
```

### Event Handling Service (agentd-hook)

Handles events from GitHub and triggers agent actions based on the event.

**Use Cases:**

- PR opened → trigger agent to review the PR
- Issue labeled → assign agent to investigate
- Commit pushed → run agent validation
- Comment added → agent responds or takes action

**Implementation:** Webhook receiver + event dispatcher

### Sub-agent Monitor (agentd-monitor)

Monitors and coordinates multiple sub-agents within a project.

**Responsibilities:**

- Track multiple agent instances per project
- Monitor resource usage
- Coordinate agent communication
- Handle agent failures and restarts

## Future Enhancements

### Advanced Agent Interaction

The following capabilities are planned for future iterations:

#### Programmatic Agent I/O

- Send input to running agents via API
- Receive structured output from agents
- Stream agent responses in real-time
- Programmatic control of agent sessions

#### Agent-to-Agent Communication

- Message passing between agents
- Shared state management
- Coordination protocols
- Event broadcasting

#### Multi-Agent Coordination

- Multiple agents per project
- Git worktrees per agent
- Separate tmux windows/panes
- Agent task delegation
- Hierarchical agent structures

### Enhanced Notification Features

- Custom menu bar application (richer than system notifications)
- In-notification code preview
- Interactive forms within notifications
- Notification grouping and prioritization

### Context-Aware Orchestration

- Git branch/status awareness
- Project dependency tracking
- Time-of-day heuristics
- User presence detection
- Integration with calendar/focus modes

### Learning and Adaptation

- Learn user preferences over time
- Suggest optimal agent configurations
- Auto-start common workflows
- Smart notification timing
