#!/usr/bin/env bash
# Escalate a rebase/restack conflict: abort, identify, label, report.
#
# Usage: conflict-escalate.sh [<branch>]
#
# If no branch is provided, uses the current branch.
#
# Actions:
#   1. Aborts any in-progress rebase
#   2. Identifies the affected branch and PR
#   3. Adds `needs-restack` label to the PR
#   4. Outputs JSON with conflict details for the conductor to post
set -euo pipefail

REPO="geoffjay/agentd"

git rebase --abort 2>/dev/null || true

BRANCH="${1:-$(git branch --show-current)}"
AFFECTED_PR=$(gh pr list --repo "$REPO" --head "$BRANCH" \
  --json number --jq '.[0].number // empty' 2>/dev/null || echo "")

if [ -n "$AFFECTED_PR" ]; then
  gh pr edit "$AFFECTED_PR" --repo "$REPO" --add-label "needs-restack"
fi

echo "=== Stack state ==="
git-spice log short

jq -n \
  --arg branch "$BRANCH" \
  --arg affected_pr "${AFFECTED_PR:-unknown}" \
  '{branch: $branch, affected_pr: $affected_pr, action: "needs-restack label applied, escalate to engineering"}'
