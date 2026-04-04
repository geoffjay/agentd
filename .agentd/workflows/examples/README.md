# Ask Service Example Workflows

This directory contains example workflow pairs demonstrating the agent-to-human Q&A pattern using the redesigned ask service.

Each example consists of two workflows:
1. **Ask workflow** — an agent creates a question for the human user
2. **Response workflow** — fires automatically when the human answers, via `ask_response` trigger

## Examples

### 1. Daily Nutrition Check

**Files:** `daily-nutrition-ask.yml` + `daily-nutrition-analyze.yml`

**Pattern:** Cron → Ask → ask_response → Analysis

The dietician agent asks the user what they ate each morning at 10am. When
the user answers, the response workflow automatically analyzes their dietary
intake and provides personalized recommendations.

**Trigger chain:**
```
Cron (10am daily) → dietician asks nutrition question
                  → user answers via CLI or UI
                  → ask_response fires → dietician analyzes and notifies
```

### 2. Deployment Approval Gate

**Files:** `deploy-approval-ask.yml` + `deploy-approval-respond.yml`

**Pattern:** Manual → Ask (urgent) → ask_response → Deploy or Abort

The deploy agent requests explicit human approval before executing a
production deployment. The operator's YES/NO response is handled
automatically — proceeding with or aborting the deployment accordingly.

**Trigger chain:**
```
Manual dispatch → deploy agent asks urgent approval question
               → operator answers YES or NO
               → ask_response fires → deploy or abort
```

### 3. End-of-Day Productivity Review

**Files:** `productivity-check.yml` + `productivity-followup.yml`

**Pattern:** Cron (weekdays) → Ask → ask_response → Summary + Plan

The productivity agent asks the user a reflective end-of-day question each
weekday at 4pm. When the user answers, the response workflow synthesizes a
summary of wins and blockers and suggests tomorrow's priorities.

**Trigger chain:**
```
Cron (4pm Mon-Fri) → productivity agent asks review question
                   → user answers at their convenience
                   → ask_response fires → agent delivers summary + tomorrow's plan
```

## Quick Start

Deploy a workflow pair:

```bash
# Deploy both workflows in a pair together
agent apply .agentd/workflows/examples/daily-nutrition-ask.yml
agent apply .agentd/workflows/examples/daily-nutrition-analyze.yml

# Enable them via the API
agent orchestrator enable-workflow <daily-nutrition-ask-id>
agent orchestrator enable-workflow <daily-nutrition-analyze-id>
```

All example workflows ship with `enabled: false` to prevent accidental
activation. Enable them explicitly after reviewing the configuration.

## ask_response Trigger Reference

The `ask_response` trigger fires when any question matching the filter criteria
is answered or dismissed. All filters are optional.

```yaml
source:
  type: ask_response
  agent_id: my-agent     # optional: only react to this agent's questions
  category: health       # optional: only react to this category
  response_pattern: "yes|approve"  # optional: regex match on answer text
```

### Template Variables

| Variable | Description |
|---|---|
| `{{question_id}}` | UUID of the answered question |
| `{{agent_id}}` | Agent that asked the question |
| `{{category}}` | Question category (if set) |
| `{{question}}` | The original question text |
| `{{answer}}` | The human's answer (empty if dismissed) |
| `{{event_type}}` | `"answered"` or `"dismissed"` |
| `{{workflow_id}}` | Workflow that created the question (if set) |

## Further Reading

- Agent skill: `.claude/skills/agent-ask/SKILL.md`
- Ask service API: `crates/ask/src/api.rs`
- Trigger strategy: `crates/orchestrator/src/scheduler/strategy.rs`
