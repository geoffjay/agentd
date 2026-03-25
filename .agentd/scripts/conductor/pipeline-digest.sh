#!/usr/bin/env bash
# Generate a pipeline status digest.
#
# Usage: pipeline-digest.sh
#
# Outputs a formatted digest suitable for posting to the operations room.
set -euo pipefail

REPO="geoffjay/agentd"

MERGE_QUEUE=$(gh pr list --repo "$REPO" --label merge-ready --json number,title | jq length)
IN_REVIEW=$(gh pr list --repo "$REPO" --label review-agent --json number,title | jq length)
NEEDS_REWORK=$(gh pr list --repo "$REPO" --label needs-rework --json number,title | jq length)
ACTIVE_BRANCHES=$(git branch -r | grep -v HEAD | wc -l | tr -d ' ')

cat <<EOF
Pipeline digest:
- Merge queue: ${MERGE_QUEUE} PR(s) ready
- In review:   ${IN_REVIEW} PR(s) awaiting reviewer
- Needs rework: ${NEEDS_REWORK} PR(s) with requested changes
- Active branches: ${ACTIVE_BRANCHES}
- Last run: $(date -u '+%Y-%m-%d %H:%M UTC')
EOF
