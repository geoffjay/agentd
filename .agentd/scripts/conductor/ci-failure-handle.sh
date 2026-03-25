#!/usr/bin/env bash
# Handle CI failure on a merge-ready PR: remove label, comment, notify.
#
# Usage: ci-failure-handle.sh <pr-number> <check-name> <state>
#
# Actions:
#   1. Removes `merge-ready` label
#   2. Comments on the PR with the failing check details
#   3. Prints a message suitable for posting to the operations room
set -euo pipefail

PR_NUMBER="${1:?Usage: ci-failure-handle.sh <pr-number> <check-name> <state>}"
CHECK_NAME="${2:?Usage: ci-failure-handle.sh <pr-number> <check-name> <state>}"
CHECK_STATE="${3:?Usage: ci-failure-handle.sh <pr-number> <check-name> <state>}"
REPO="geoffjay/agentd"

gh pr edit "$PR_NUMBER" --repo "$REPO" --remove-label "merge-ready"

gh pr comment "$PR_NUMBER" --repo "$REPO" --body \
  "CI check failed: **${CHECK_NAME}** (state: ${CHECK_STATE}). Removing merge-ready label.
Fix the failing check and re-apply merge-ready when CI is green."

echo "PR #${PR_NUMBER} removed from merge queue: CI check '${CHECK_NAME}' failed (${CHECK_STATE})."
