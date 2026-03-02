#!/usr/bin/env bash
#
# Launch a worker agent with a workflow that monitors GitHub issues labeled
# "agent" on geoffjay/agentd and dispatches them one at a time.
#
# The agent receives each issue as a prompt and works on it in the agentd
# repo. Once it finishes (or errors), the workflow picks up the next
# undispatched issue on the next poll cycle.
#
# Prerequisites:
#   - orchestrator running: cargo run -p orchestrator
#   - gh CLI authenticated: gh auth status
#   - issues exist with the "agent" label on geoffjay/agentd
#
# Usage:
#   ./scripts/launch-worker.sh
#   ./scripts/launch-worker.sh --label "agent-ready"   # custom label
#   ./scripts/launch-worker.sh --interval 120           # poll every 2 min
#

set -euo pipefail

API="http://127.0.0.1:17006"
LABEL="agent"
INTERVAL=60
OWNER="geoffjay"
REPO="agentd"

# Parse arguments.
while [[ $# -gt 0 ]]; do
    case "$1" in
        --label)    LABEL="$2";    shift 2 ;;
        --interval) INTERVAL="$2"; shift 2 ;;
        --owner)    OWNER="$2";    shift 2 ;;
        --repo)     REPO="$2";     shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "==> Checking orchestrator health..."
health=$(curl -sf "$API/health") || {
    echo "ERROR: orchestrator not reachable at $API"
    echo "Start it with: cargo run -p orchestrator"
    exit 1
}
echo "    $health"

# -- Step 1: Create the worker agent --
#
# No initial prompt — the workflow will send prompts as issues arrive.
# The system prompt defines how the agent should approach each issue.

SYSTEM_PROMPT=$(cat <<'SYSPROMPT'
You are a worker agent for the agentd project. You will receive GitHub issues
as tasks. For each issue:

1. Read the issue title, body, and labels carefully
2. Understand what is being asked
3. Plan your approach before writing code
4. Implement the change in the working directory
5. Run any relevant tests to verify your work
6. Create a git branch and commit your changes with a descriptive message
7. Push the branch and create a pull request using the gh CLI

Guidelines:
- Keep changes focused on the issue scope — don't refactor unrelated code
- Write tests for new functionality
- Follow existing code conventions in the project
- If an issue is unclear or too large, comment on it with questions instead
  of guessing: gh issue comment <number> --repo geoffjay/agentd --body "..."
- After completing work, close the issue:
  gh issue close <number> --repo geoffjay/agentd

Use worktrees or branches to isolate your work for each issue.
SYSPROMPT
)

echo ""
echo "==> Creating worker agent..."
AGENT_RESPONSE=$(curl -sf -X POST "$API/agents" \
    -H "Content-Type: application/json" \
    -d "$(jq -n \
        --arg name "worker" \
        --arg working_dir "/Users/geoff/Projects/agentd" \
        --arg user "geoff" \
        --arg shell "/bin/zsh" \
        --arg system_prompt "$SYSTEM_PROMPT" \
        '{
            name: $name,
            working_dir: $working_dir,
            user: $user,
            shell: $shell,
            worktree: false,
            system_prompt: $system_prompt
        }'
    )")

AGENT_ID=$(echo "$AGENT_RESPONSE" | jq -r '.id')
AGENT_STATUS=$(echo "$AGENT_RESPONSE" | jq -r '.status')

echo "    Agent ID:     $AGENT_ID"
echo "    Status:       $AGENT_STATUS"
echo "    Tmux session: $(echo "$AGENT_RESPONSE" | jq -r '.tmux_session // "pending"')"

# -- Step 2: Wait for the agent to connect --
#
# The agent needs a moment to start in tmux and connect via WebSocket.
# The workflow creation validates that the agent is Running.

echo ""
echo "==> Waiting for agent to be ready..."
for i in $(seq 1 30); do
    sleep 2
    status=$(curl -sf "$API/agents/$AGENT_ID" | jq -r '.status')
    if [[ "$status" == "running" ]]; then
        # Check WebSocket connectivity via health endpoint
        active=$(curl -sf "$API/health" | jq -r '.agents_active')
        if [[ "$active" -gt 0 ]]; then
            echo "    Agent is running and connected (attempt $i)"
            break
        fi
    fi
    if [[ "$status" == "failed" ]]; then
        echo "ERROR: Agent failed to start"
        exit 1
    fi
    echo "    Waiting... (status: $status, attempt $i/30)"
done

# -- Step 3: Create the workflow --
#
# The prompt template uses placeholders that get filled from each GitHub issue.
# Available: {{title}}, {{body}}, {{url}}, {{labels}}, {{assignee}}, {{source_id}}

PROMPT_TEMPLATE=$(cat <<'TEMPLATE'
Work on the following GitHub issue:

Issue #{{source_id}}: {{title}}
URL: {{url}}
Labels: {{labels}}

Description:
{{body}}

Instructions:
1. Create a feature branch from the main branch: git checkout -b issue-{{source_id}}
2. Implement the changes described in the issue
3. Run tests to verify: cargo test
4. Commit with a descriptive message referencing the issue
5. Push and create a PR: gh pr create --repo geoffjay/agentd --title "{{title}}" --body "Closes #{{source_id}}"
6. Close the issue: gh issue close {{source_id}} --repo geoffjay/agentd
TEMPLATE
)

echo ""
echo "==> Creating workflow (polling $OWNER/$REPO for label '$LABEL' every ${INTERVAL}s)..."
WORKFLOW_RESPONSE=$(curl -sf -X POST "$API/workflows" \
    -H "Content-Type: application/json" \
    -d "$(jq -n \
        --arg name "issue-worker" \
        --arg agent_id "$AGENT_ID" \
        --arg owner "$OWNER" \
        --arg repo "$REPO" \
        --arg label "$LABEL" \
        --arg prompt_template "$PROMPT_TEMPLATE" \
        --argjson interval "$INTERVAL" \
        '{
            name: $name,
            agent_id: $agent_id,
            source_config: {
                type: "github_issues",
                owner: $owner,
                repo: $repo,
                labels: [$label],
                state: "open"
            },
            prompt_template: $prompt_template,
            poll_interval_secs: $interval,
            enabled: true
        }'
    )")

WORKFLOW_ID=$(echo "$WORKFLOW_RESPONSE" | jq -r '.id')

echo "    Workflow ID:   $WORKFLOW_ID"
echo "    Polling:       $OWNER/$REPO (label: $LABEL)"
echo "    Interval:      ${INTERVAL}s"
echo "    Enabled:       $(echo "$WORKFLOW_RESPONSE" | jq -r '.enabled')"

echo ""
echo "==> Worker is running. The workflow will poll for issues and dispatch them."
echo ""
echo "Monitor agent:"
echo "  curl -s $API/agents/$AGENT_ID | jq"
echo ""
echo "Send a message to the agent:"
echo "  curl -s -X POST $API/agents/$AGENT_ID/message \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"content\": \"your message here\"}'"
echo ""
echo "Monitor workflow:"
echo "  curl -s $API/workflows/$WORKFLOW_ID | jq"
echo ""
echo "Dispatch history:"
echo "  curl -s $API/workflows/$WORKFLOW_ID/history | jq"
echo ""
echo "Watch this agent's output:"
echo "  websocat ws://127.0.0.1:17006/stream/$AGENT_ID"
echo ""
echo "Watch all agent output:"
echo "  websocat ws://127.0.0.1:17006/stream"
echo ""
echo "Attach to tmux session:"
echo "  tmux attach -t agentd-orch-$AGENT_ID"
echo ""
echo "Pause workflow:"
echo "  curl -s -X PUT $API/workflows/$WORKFLOW_ID -H 'Content-Type: application/json' -d '{\"enabled\":false}'"
echo ""
echo "Teardown:"
echo "  curl -X DELETE $API/workflows/$WORKFLOW_ID"
echo "  curl -X DELETE $API/agents/$AGENT_ID"
