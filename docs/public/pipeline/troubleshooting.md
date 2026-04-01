# Pipeline Troubleshooting

Common failure modes in the autonomous pipeline and how to resolve them.

---

## PR stuck in `needs-rework`

**Symptom:** A PR has had `needs-rework` for more than a few hours with no worker
activity.

**What should happen:** The conductor's 5-minute sync re-dispatches the worker after
15 minutes by removing `needs-rework` from the PR and re-applying `agent` to
the linked issue.

**Check:**

```bash
# Is the conductor running?
agent orchestrator list-agents --status running

# Has needs-rework been on this PR for >15 min?
gh pr view <number> --repo geoffjay/agentd --json labels,updatedAt

# Is the linked issue already dispatched?
gh issue view <issue-number> --repo geoffjay/agentd --json labels
```

**Resolution:**

If the conductor hasn't re-dispatched, manually re-dispatch:

```bash
gh pr edit <pr-number> --repo geoffjay/agentd --remove-label "needs-rework"
gh issue edit <issue-number> --repo geoffjay/agentd --add-label "agent"
```

If the worker keeps producing the same issue, read the review comments and address
the root cause before re-dispatching:

```bash
gh pr review list <pr-number> --repo geoffjay/agentd
```

---

## Restack conflict (`needs-restack`) {#restack-conflict}

**Symptom:** A PR has `needs-restack`. The conductor posted an escalation to
`#engineering`.

**What happened:** After a parent PR was merged, `git-spice repo sync` tried to
rebase the stacked branch but hit a conflict it could not resolve automatically.
The conductor never resolves merge conflicts - it always escalates.

**Diagnose:**

```bash
git fetch origin
git checkout <branch-with-conflict>
git-spice log short   # see where the branch sits in the stack
git status            # after a failed rebase
```

**Resolution (worker or human):**

```bash
# 1. Sync to see current state
git-spice repo sync

# 2. Rebase the conflicting branch onto its updated parent
git-spice branch restack

# 3. Resolve conflicts if any
git status
# ... edit conflicting files ...
git add <resolved-files>
git rebase --continue

# 4. Resubmit the PR
git-spice branch submit --fill --no-prompt --label review-agent

# 5. Remove the needs-restack label
gh pr edit <number> --repo geoffjay/agentd --remove-label "needs-restack"
```

---

## Stale PR - no activity for days

**Symptom:** The conductor posted to `#operations` that a PR or issue has had no
activity for >3 days.

**Common causes:**
- Worker was dispatched but never started (agent not running)
- Review was requested but no reviewer picked it up
- `needs-rework` was applied but worker didn't respond
- CI is broken and the developer hasn't noticed

**Check:**

```bash
# Is the relevant agent running?
agent orchestrator list-agents --status running

# What is the PR/issue state?
gh pr view <number> --repo geoffjay/agentd --json labels,updatedAt,statusCheckRollup
gh issue view <number> --repo geoffjay/agentd --json labels,updatedAt
```

**Resolution:**

=== "Agent not running"
    ```bash
    # Redeploy the agent stack
    agent apply .agentd/

    # Then re-trigger by removing and re-adding the dispatch label
    gh issue edit <number> --repo geoffjay/agentd --remove-label "agent"
    gh issue edit <number> --repo geoffjay/agentd --add-label "agent"
    ```

=== "CI broken"
    ```bash
    # Check which check is failing
    gh pr checks <number> --repo geoffjay/agentd

    # Fix CI, then re-apply merge-ready when green
    gh pr edit <number> --repo geoffjay/agentd --add-label "merge-ready"
    ```

=== "Review stalled"
    ```bash
    # Manually trigger review-agent dispatch
    gh pr edit <number> --repo geoffjay/agentd --remove-label "review-agent"
    gh pr edit <number> --repo geoffjay/agentd --add-label "review-agent"
    ```

---

## CI failure blocking merge queue

**Symptom:** The conductor removed `merge-ready` from a PR with a comment about CI
failure. The merge queue is blocked.

**What happened:** One of the CI checks (`FAILURE`, `ERROR`, or `TIMED_OUT`) was
detected during the merge flow. The conductor removes `merge-ready` and stops - it
does not retry automatically.

**Check:**

```bash
# Which check failed?
gh pr checks <number> --repo geoffjay/agentd

# Structured output
gh pr view <number> --repo geoffjay/agentd --json statusCheckRollup \
  --jq '[.statusCheckRollup[] | {name: .name, state: .state}]'
```

**Resolution:**

1. Fix the root cause (flaky test, build error, lint failure)
2. Push a commit to re-trigger CI
3. When all checks pass, re-apply `merge-ready`:

```bash
gh pr edit <number> --repo geoffjay/agentd --add-label "merge-ready"
```

---

## Dispatch deduplication - re-triggering an agent

**Symptom:** You applied a dispatch label (`agent`, `research-agent`, etc.) but
the agent never picked up the issue. Re-applying the label has no effect.

**What happened:** The orchestrator's scheduler deduplicates on
`(workflow_id, source_id)`. Once a dispatch record exists for an issue, re-applying
the label does not create a second dispatch.

**Check:**

```bash
# List recent dispatch history for the workflow
agent orchestrator list-dispatches --workflow <workflow-name>
```

**Resolution:**

Remove the label first, then re-add it. This creates a new GitHub label event that
the scheduler treats as a fresh source item (the dedup record is cleared when the
previous dispatch reaches `completed` or `failed`):

```bash
gh issue edit <number> --repo geoffjay/agentd --remove-label "research-agent"
# Wait a moment for the event to propagate
gh issue edit <number> --repo geoffjay/agentd --add-label "research-agent"
```

If the dispatch never completed (e.g., the agent crashed), the dedup record may
still block re-dispatch. Clear it via the API:

```bash
agent orchestrator clear-dispatch --workflow <workflow-id> --source <issue-number>
```

---

## Merge conflict - base branch diverged

**Symptom:** GitHub shows `CONFLICTING` on a `merge-ready` PR. The conductor added
`needs-restack` and escalated to `#engineering`.

**What happened:** The base branch has diverged since the branch was created (another
PR merged into it that touches the same files).

**Resolution:**

The worker (or a human) must resolve the conflict:

```bash
git fetch origin
git checkout <branch>
git-spice branch restack   # rebases onto updated parent

# If conflicts appear:
git status
# ... resolve conflicts ...
git add <files>
git rebase --continue

# Resubmit
git-spice branch submit --fill --no-prompt --label review-agent
gh pr edit <number> --repo geoffjay/agentd --remove-label "needs-restack"
```

---

## Agent dispatched but never responded

**Symptom:** An issue has a dispatch label (`agent`, etc.) applied over 6 hours
ago but the agent has posted nothing and opened no PR.

**Check:**

```bash
# Is the agent's workflow enabled and running?
agent orchestrator list-workflows

# Is the agent itself running?
agent orchestrator list-agents --status running

# Check the agent's tmux session
agent orchestrator attach --name worker
# Detach: Ctrl-b d
```

**Common causes and fixes:**

=== "Agent not running"
    ```bash
    agent apply .agentd/agents/worker.yml
    ```

=== "Workflow disabled"
    ```bash
    curl -s -X PATCH http://127.0.0.1:17006/workflows/<id> \
      -H "Content-Type: application/json" \
      -d '{"enabled": true}'
    ```

=== "Agent stuck on previous task"
    ```bash
    # Attach to see what it's doing
    agent orchestrator attach --name worker
    # Send a message to interrupt if needed
    agent orchestrator send-message <id> "Stop current task. New priority task incoming."
    ```

=== "Dispatch already recorded (dedup)"
    See [Dispatch deduplication](#dispatch-deduplication--re-triggering-an-agent) above.

---

## Workflow chain not firing

**Symptom:** Triage completed but `enrich-agent` was never applied. Or review
completed but `merge-ready` was never applied.

**What happened:** The chain workflows (`triage-enrich-chain`, `review-merge-chain`)
ship disabled and require `source_workflow_id` to be configured before enabling.

**Check:**

```bash
# Is the chain workflow enabled?
agent orchestrator list-workflows --json | \
  jq '.[] | select(.name | test("chain")) | {name, enabled}'

# Does it have source_workflow_id set?
agent orchestrator list-workflows --json | \
  jq '.[] | select(.name=="triage-enrich-chain") | .source'
```

**Resolution:**

See [Workflow chaining setup](conductor.md#triage-enrich-chain) for the full
configuration steps.

---

## Conductor not running / pipeline stalled

**Symptom:** Issues are accumulating with no state transitions. The `#operations`
room has received no digest in >10 minutes.

**Check:**

```bash
# Is the conductor agent running?
agent orchestrator list-agents --json | jq '.[] | select(.name=="conductor")'

# Is the conductor-sync workflow enabled?
agent orchestrator list-workflows --json | \
  jq '.[] | select(.name=="conductor-sync") | {name, enabled}'

# Are the services up?
agent status
```

**Resolution:**

```bash
# Redeploy the full stack
agent teardown .agentd/
agent apply .agentd/

# Or just the conductor
agent apply .agentd/agents/conductor.yml
agent apply .agentd/workflows/conductor-sync.yml

# Manually trigger a sync to catch up
gh issue edit <any-open-issue> --repo geoffjay/agentd --add-label "conductor-sync"
```

---

## Related

- [Pipeline Overview](index.md) - Architecture and agent roster
- [Conductor Behavior](conductor.md) - Sync protocol and merge queue
- [State Machine](../pipeline-state-machine.md) - Label reference
- [Human Approval Gates](../../planning/autonomous-pipeline-gates.md) - Gate reference
