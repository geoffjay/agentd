---
name: create-plan-issue
description: Create a new GitHub plan issue with the "Plan: <title>" format, appropriate labels, and structured body. Use when the user wants to capture a high-level plan for future work.
---

# Create Plan Issue

Creates a GitHub issue following the project's plan issue conventions. Plan issues describe a problem and proposed solution at a high level; the `plan-agent` label signals an agent to pick it up for detailed planning.

## Template

The issue body is based on `.claude/skills/create-plan-issue/plan-issue-template.md`:

```markdown
## Problem

<what the problem is or why this work is needed>

## Proposed Solution

<how you think it should be solved at a high level>

## Considerations

<optional: constraints, open questions, alternatives, related issues>
```

## Workflow

### Step 1 — Gather the title

If the user invoked the skill with an argument (e.g. `/create-plan-issue add foo to do bar`), use that as the working title. Otherwise ask:

```
AskUserQuestion:
  question: "What should this plan issue be about? (used as the title after 'Plan: ')"
```

### Step 2 — Gather body content

Ask the following in a **single** AskUserQuestion call (up to 4 questions at once):

```
AskUserQuestion questions:
  1. "What is the problem or motivation for this work?"
     header: "Problem"
     options: [let the user type a custom answer via Other]
     → collect as free-text (provide 2-3 placeholder options drawn from the title context)

  2. "How do you think this should be solved?"
     header: "Solution"
     options: [let the user type a custom answer via Other]
     → collect as free-text

  3. "Which category labels apply?" (multiSelect: true)
     header: "Labels"
     options:
       - enhancement  (new feature or improvement)
       - bug          (something isn't working)
       - refactor     (code improvement, no behavior change)
       - frontend     (frontend / UI work)
       - architecture (cross-service design)
       - research     (investigation or spike)
       - security     (security hardening or audit)
       - documentation (docs work)

  4. "Add 'plan-agent' label now to queue it for automated planning?"
     header: "Plan agent"
     options:
       - Yes — add plan-agent now (Recommended)
       - No — add it manually later
```

### Step 3 — Handle considerations (optional)

If the problem or solution text hints at open questions or trade-offs, offer one more question:

```
AskUserQuestion:
  question: "Any additional considerations, constraints, or related issues to note?"
  options:
    - Skip — none right now
    - <let the user type via Other>
```

Only ask this if the earlier answers suggest it would add value; skip it for straightforward issues.

### Step 4 — Confirm and create

Show a brief summary of what will be created, then create the issue:

```bash
# Build label list: always include "plan", add "plan-agent" if selected, plus chosen category labels
gh issue create \
  --title "Plan: <title>" \
  --body "$(cat <<'EOF'
## Problem

<problem text>

## Proposed Solution

<solution text>

## Considerations

<considerations text, or omit section if skipped>
EOF
)" \
  --label "plan" \
  --label "plan-agent" \     # if selected
  --label "enhancement"      # repeat --label for each chosen category label
```

Return the issue URL to the user when done.

## Label Reference

| Label | When to use |
|---|---|
| `plan` | Always — identifies this as a plan issue |
| `plan-agent` | Add when ready for the planning agent to pick it up |
| `enhancement` | New feature or improvement |
| `bug` | Something broken that needs fixing |
| `refactor` | Code cleanup without behavior change |
| `frontend` | UI / frontend work |
| `architecture` | Cross-service or structural design |
| `research` | Investigation, spike, or feasibility study |
| `security` | Security audit, hardening, or vulnerability |
| `documentation` | Docs additions or improvements |

## Examples

Existing plan issues in this repo for reference:
- #1173 Plan: Add PROJECT.md equivalent for projects
- #221  Plan for PostgreSQL support in core service (labels: plan, research, architecture)
- #312  Plan to edit the agent configuration (label: plan)
