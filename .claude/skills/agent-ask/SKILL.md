---
name: agent-ask
description: Ask the human user a question during workflow execution and check for answers. Use for agent-to-human Q&A, collecting decisions, and reacting to human responses via ask_response workflow triggers.
---

# Agent Ask

Skill for interacting with the agentd ask service — a purpose-built agent-to-human question/answer system. Agents ask questions during workflow execution; humans answer at their convenience; the orchestrator can trigger follow-up workflows on responses.

## Asking a Question

Create a question for the human user during workflow execution:

```bash
# Basic question
agent ask create \
  --agent-id <my-agent-id> \
  --question "Should I proceed with the deployment to production?"

# With category, context, priority, and expiry
agent ask create \
  --agent-id dietician \
  --question "What did you eat yesterday?" \
  --category health \
  --context "Daily nutrition tracking for your meal plan" \
  --priority normal \
  --expires-in 86400
```

Priority levels: `low`, `normal` (default), `high`, `urgent`

The command returns the question UUID — store it if you need to poll for an answer.

## Checking for Answers

List questions filtered by agent and status:

```bash
# See all answered questions for this agent
agent ask list --agent-id <my-agent-id> --status Answered

# See all pending (unanswered) questions for this agent
agent ask list --agent-id <my-agent-id> --status Pending

# Filter by category
agent ask list --agent-id dietician --category health --limit 10
```

Get a specific question (including its answer):

```bash
agent ask get <question-uuid>
```

The `answer` field in the response contains the human's response when `status` is `Answered`.

## Answering Questions (Human-Facing)

These commands are for the human operator, but agents can invoke them in test/demo scenarios:

```bash
# Answer a pending question
agent ask answer <question-uuid> "I had salad for lunch and pasta for dinner"

# Dismiss a question without answering
agent ask dismiss <question-uuid>
```

## Health Check

```bash
agent ask health
```

## Workflow Integration — ask_response Trigger

The most powerful pattern is using the `ask_response` trigger type in your workflow config. The orchestrator automatically fires the workflow when a question is answered or dismissed — no polling required.

Example workflow YAML:

```yaml
name: nutrition-followup
agent_id: dietician
trigger:
  type: ask_response
  agent_id: dietician          # optional: only react to this agent's questions
  category: health             # optional: only react to health-category questions
  response_pattern: "salad|vegetable|fruit"  # optional: regex match on answer text
prompt_template: |
  The human answered your nutrition question:
  Question: {{ question }}
  Answer: {{ answer }}

  Based on this response, provide personalized nutrition advice.
```

### Template Variables for ask_response Workflows

These variables are available in the prompt template when triggered by an ask response:

| Variable | Description |
|---|---|
| `{{ question_id }}` | UUID of the answered question |
| `{{ agent_id }}` | Agent that asked the question |
| `{{ category }}` | Question category (if set) |
| `{{ question }}` | The original question text |
| `{{ answer }}` | The human's answer (empty if dismissed) |
| `{{ event_type }}` | `"answered"` or `"dismissed"` |
| `{{ workflow_id }}` | Workflow that created the question (if set) |

## Best Practices

- **Use meaningful categories** — categories enable targeted filtering with `ask list --category` and targeted workflow triggers with `trigger.category`
- **Set appropriate priority** — `urgent` should be reserved for blocking decisions; use `normal` for routine questions
- **Provide context** — the `--context` flag explains *why* the question is being asked, helping the human give a useful answer
- **Set expiration for time-sensitive questions** — use `--expires-in <seconds>` so stale questions auto-expire (e.g., `3600` for 1 hour, `86400` for 1 day)
- **Prefer ask_response triggers over polling** — instead of looping with `ask list`, configure a workflow that fires automatically when the question is answered
- **One question at a time per topic** — avoid creating many unanswered questions simultaneously; the human's attention is limited
