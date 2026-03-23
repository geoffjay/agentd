#!/usr/bin/env bash
# Post-merge restack: sync git-spice and resubmit upstack PRs.
#
# Usage: post-merge-restack.sh [<merged-pr-number>]
#
# If git-spice repo sync fails due to a rebase conflict, this script:
#   1. Aborts the rebase
#   2. Identifies the conflicting branch and its PR
#   3. Adds `needs-restack` label to the blocked PR
#   4. Prints conflict details for the conductor to escalate
#
# Exit codes:
#   0 - restack succeeded
#   2 - restack conflict (details printed to stdout as JSON)
set -euo pipefail

MERGED_PR="${1:-}"
REPO="geoffjay/agentd"

echo "=== Syncing git-spice ==="
if git-spice repo sync; then
  echo "=== Resubmitting upstack PRs ==="
  git-spice stack submit --fill --no-prompt 2>&1 || true
  echo '{"status": "ok"}'
  exit 0
fi

# Restack failed — handle the conflict
echo "Restack conflict detected, aborting rebase..." >&2
git rebase --abort 2>/dev/null || true

BRANCH=$(git branch --show-current)
AFFECTED_PR=$(gh pr list --repo "$REPO" --head "$BRANCH" \
  --json number --jq '.[0].number // empty' 2>/dev/null || echo "")

if [ -n "$AFFECTED_PR" ]; then
  gh pr edit "$AFFECTED_PR" --repo "$REPO" --add-label "needs-restack"
fi

# Output conflict details as JSON for the conductor to use in escalation
jq -n \
  --arg status "conflict" \
  --arg branch "$BRANCH" \
  --arg affected_pr "${AFFECTED_PR:-unknown}" \
  --arg merged_pr "${MERGED_PR:-unknown}" \
  '{status: $status, branch: $branch, affected_pr: $affected_pr, merged_pr: $merged_pr}'

exit 2
