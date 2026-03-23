#!/usr/bin/env bash
# Detect stale PRs and issues.
#
# Usage: stale-check.sh
#
# Outputs JSON arrays for:
#   - PRs with no activity for >7 days
#   - Issues with `agent` label open for >3 days
set -euo pipefail

REPO="geoffjay/agentd"

echo "=== Stale PRs (no activity >7 days) ==="
gh pr list --repo "$REPO" --state open \
  --json number,title,updatedAt,labels \
  --jq '[.[] | select((.updatedAt | fromdateiso8601) < (now - 604800))]'

echo ""
echo "=== Stale agent issues (open >3 days) ==="
gh issue list --repo "$REPO" --label agent --state open \
  --json number,title,updatedAt \
  --jq '[.[] | select((.updatedAt | fromdateiso8601) < (now - 259200))]'
