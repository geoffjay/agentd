#!/usr/bin/env bash
# Conductor session start: sync git-spice state and check the merge queue.
#
# Usage: session-sync.sh
#
# Outputs:
#   - git-spice sync results
#   - Current stack state
#   - Merge-ready PRs sorted by base branch
#   - CI-failing PRs in the merge queue
set -euo pipefail

REPO="geoffjay/agentd"

echo "=== Syncing git-spice state ==="
git-spice repo sync

echo ""
echo "=== Stack state ==="
git-spice log short

echo ""
echo "=== Merge queue (sorted by base branch) ==="
# Note: sort_by(.baseRefName) is an alphabetical approximation of stack order
# (e.g. "feature/autonomous-pipeline" sorts before "issue-NNN"), not a true
# topological sort. Use `git-spice log short` for authoritative stack order.
gh pr list --repo "$REPO" --label merge-ready \
  --json number,title,baseRefName,headRefName,reviews,statusCheckRollup \
  --jq 'sort_by(.baseRefName)'

echo ""
echo "=== CI-failing PRs in merge queue ==="
gh pr list --repo "$REPO" --label merge-ready \
  --json number,title,statusCheckRollup \
  --jq '[.[] | select(
    [.statusCheckRollup[] | select(.state | IN("FAILURE","ERROR","TIMED_OUT"))] | length > 0
  )]'
