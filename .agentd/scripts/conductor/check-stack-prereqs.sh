#!/usr/bin/env bash
# Check whether a PR's stack prerequisites are met for merging.
#
# Usage: check-stack-prereqs.sh <pr-number>
#
# Checks if the PR's base branch is a trunk branch (main or
# feature/autonomous-pipeline) or has an already-merged PR.
#
# Exit codes:
#   0 - prerequisites met (safe to merge)
#   1 - prerequisites not met (base PR not yet merged)
#
# Outputs JSON with base branch info.
set -euo pipefail

PR_NUMBER="${1:?Usage: check-stack-prereqs.sh <pr-number>}"
REPO="geoffjay/agentd"

BASE_BRANCH=$(gh pr view "$PR_NUMBER" --repo "$REPO" --json baseRefName --jq '.baseRefName')

# Trunk branches are always OK
if [ "$BASE_BRANCH" = "main" ] || [ "$BASE_BRANCH" = "feature/autonomous-pipeline" ]; then
  jq -n --arg base "$BASE_BRANCH" '{ready: true, base: $base, reason: "trunk branch"}'
  exit 0
fi

# Check if the base branch has a merged PR
MERGED_BASE=$(gh pr list --repo "$REPO" --head "$BASE_BRANCH" --state merged \
  --json number,title --jq '.[0] // empty')

if [ -n "$MERGED_BASE" ]; then
  echo "$MERGED_BASE" | jq --arg base "$BASE_BRANCH" '. + {ready: true, base: $base, reason: "base PR merged"}'
  exit 0
fi

# Base PR not yet merged
OPEN_BASE=$(gh pr list --repo "$REPO" --head "$BASE_BRANCH" --state open \
  --json number,title --jq '.[0] // empty')

if [ -n "$OPEN_BASE" ]; then
  echo "$OPEN_BASE" | jq --arg base "$BASE_BRANCH" '. + {ready: false, base: $base, reason: "base PR still open — merge it first"}'
else
  jq -n --arg base "$BASE_BRANCH" '{ready: false, base: $base, reason: "no PR found for base branch"}'
fi

exit 1
