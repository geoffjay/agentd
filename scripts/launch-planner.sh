#!/usr/bin/env bash
#
# Launch a planning agent that analyzes the agentd project and proposes
# issues for the GitHub project board.
#
# Prerequisites:
#   - orchestrator running: cargo run -p orchestrator
#   - gh CLI authenticated: gh auth status
#
# Usage:
#   ./scripts/launch-planner.sh
#

set -euo pipefail

API="http://127.0.0.1:17006"

echo "==> Checking orchestrator health..."
health=$(curl -sf "$API/health") || {
    echo "ERROR: orchestrator not reachable at $API"
    echo "Start it with: cargo run -p orchestrator"
    exit 1
}
echo "    $health"

# -- Step 1: Create the planning agent --

SYSTEM_PROMPT=$(cat <<'SYSPROMPT'
You are a project planning agent for the agentd project. Your role is to:

1. Analyze the codebase structure, especially the crates/ directory
2. Understand the purpose and architecture of each service/crate
3. Identify gaps, improvements, and next steps
4. Propose well-structured GitHub issues

When proposing issues, format each as:
- Title: concise, actionable (e.g., "Add health check endpoint to wrap crate")
- Labels: use appropriate labels (enhancement, bug, documentation, refactor)
- Description: include context, acceptance criteria, and relevant file paths

Use the gh CLI to create issues on the geoffjay/agentd repository and add them
to the GitHub project https://github.com/users/geoffjay/projects/5.
SYSPROMPT
)

INITIAL_PROMPT=$(cat <<'PROMPT'
Analyze the agentd project to identify the purpose of each service created in the crates/ directory. For each crate:

1. Read its Cargo.toml and main source files to understand what it does
2. Identify its current state (complete, in-progress, stub)
3. Note dependencies between crates

Then, based on your analysis, propose 5-10 issues for the GitHub project at https://github.com/users/geoffjay/projects/5. Focus on:

- Missing functionality or incomplete implementations
- Documentation gaps
- Testing gaps
- Integration points between crates that need work
- Operational concerns (logging, error handling, configuration)

For each proposed issue, create it using:
  gh issue create --repo geoffjay/agentd --title "..." --body "..." --label "..."

After creating each issue, add it to the project board:
  gh project item-add 5 --owner geoffjay --url <issue-url>

Start by listing the crates/ directory and reading each crate's Cargo.toml.
PROMPT
)

echo ""
echo "==> Creating planning agent..."
AGENT_RESPONSE=$(curl -sf -X POST "$API/agents" \
    -H "Content-Type: application/json" \
    -d "$(jq -n \
        --arg name "planner" \
        --arg working_dir "/Users/geoff/Projects/agentd" \
        --arg user "geoff" \
        --arg shell "/bin/zsh" \
        --arg system_prompt "$SYSTEM_PROMPT" \
        --arg prompt "$INITIAL_PROMPT" \
        '{
            name: $name,
            working_dir: $working_dir,
            user: $user,
            shell: $shell,
            worktree: false,
            system_prompt: $system_prompt,
            prompt: $prompt
        }'
    )")

AGENT_ID=$(echo "$AGENT_RESPONSE" | jq -r '.id')
AGENT_STATUS=$(echo "$AGENT_RESPONSE" | jq -r '.status')

echo "    Agent ID:     $AGENT_ID"
echo "    Status:       $AGENT_STATUS"
echo "    Tmux session: $(echo "$AGENT_RESPONSE" | jq -r '.tmux_session // "pending"')"

echo ""
echo "==> Agent created. It will connect via WebSocket and begin working."
echo ""
echo "Monitor agent status:"
echo "  curl -s $API/agents/$AGENT_ID | jq"
echo ""
echo "Send a message to the agent:"
echo "  curl -s -X POST $API/agents/$AGENT_ID/message \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"content\": \"your message here\"}'"
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
echo "Terminate when done:"
echo "  curl -X DELETE $API/agents/$AGENT_ID"
