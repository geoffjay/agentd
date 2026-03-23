#!/usr/bin/env bash
# Check a PR's readiness for merge: metadata, CI, approvals, mergeability.
#
# Usage: pr-check.sh <pr-number>
#
# Outputs JSON with: number, title, base, head, mergeable, labels,
# approved count, changes_requested count, ci_status (PASSED/PENDING/FAILED),
# and failing check names if any.
set -euo pipefail

PR_NUMBER="${1:?Usage: pr-check.sh <pr-number>}"
REPO="geoffjay/agentd"

# Fetch all relevant PR data in a single API call
PR_DATA=$(gh pr view "$PR_NUMBER" --repo "$REPO" \
  --json number,title,baseRefName,headRefName,labels,mergeable,reviews,statusCheckRollup,url)

echo "$PR_DATA" | jq '{
  number: .number,
  title: .title,
  base: .baseRefName,
  head: .headRefName,
  url: .url,
  mergeable: .mergeable,
  labels: [.labels[].name],
  approved: ([.reviews[] | select(.state == "APPROVED")] | length),
  changes_requested: ([.reviews[] | select(.state == "CHANGES_REQUESTED")] | length),
  ci_states: ([.statusCheckRollup[]?.state] | unique),
  ci_status: (
    if [.statusCheckRollup[] | select(.state | IN("PENDING","IN_PROGRESS","QUEUED","EXPECTED"))] | length > 0
    then "PENDING"
    elif [.statusCheckRollup[] | select(.state | IN("FAILURE","ERROR","TIMED_OUT","CANCELLED","ACTION_REQUIRED"))] | length > 0
    then "FAILED"
    else "PASSED"
    end
  ),
  failing_checks: [.statusCheckRollup[] | select(.state | IN("FAILURE","ERROR","TIMED_OUT","CANCELLED","ACTION_REQUIRED")) | {name: .name, state: .state, conclusion: .conclusion}],
  approval_details: {
    approved: ([.reviews | group_by(.author.login)[]
                | sort_by(.submittedAt) | last
                | select(.state == "APPROVED")] | length),
    changes_requested: ([.reviews | group_by(.author.login)[]
                         | sort_by(.submittedAt) | last
                         | select(.state == "CHANGES_REQUESTED")] | length)
  }
}'
