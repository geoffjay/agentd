#!/usr/bin/env bash
# Squash-merge a PR and verify the result.
#
# Usage: pr-merge.sh <pr-number>
#
# This script only performs the merge itself. Stack prerequisite checks,
# CI verification, and approval checks should be done beforehand (see pr-check.sh).
# Post-merge restacking is handled by post-merge-restack.sh.
#
# Exit codes:
#   0 - merge succeeded
#   1 - merge failed
set -euo pipefail

PR_NUMBER="${1:?Usage: pr-merge.sh <pr-number>}"
REPO="geoffjay/agentd"

TITLE=$(gh pr view "$PR_NUMBER" --repo "$REPO" --json title --jq '.title')

echo "Merging PR #${PR_NUMBER}: ${TITLE}"
gh pr merge "$PR_NUMBER" --repo "$REPO" --squash --delete-branch \
  --subject "$TITLE"

# Verify the merge succeeded
STATE=$(gh pr view "$PR_NUMBER" --repo "$REPO" --json state,mergedAt \
  --jq '{state: .state, mergedAt: .mergedAt}')
echo "$STATE"

MERGED=$(echo "$STATE" | jq -r '.state')
if [ "$MERGED" != "MERGED" ]; then
  echo "ERROR: PR #${PR_NUMBER} state is '${MERGED}', expected 'MERGED'" >&2
  exit 1
fi

echo "PR #${PR_NUMBER} merged successfully."
