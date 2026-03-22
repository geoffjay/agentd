---
name: git-spice
description: Branch stacking and PR management with git-spice for the agentd project. Use when creating stacked branches, submitting PRs, syncing with upstream, resolving rebase conflicts, or navigating a branch stack.
---

# git-spice

Reference skill for using git-spice (`git-spice`) in the agentd project. git-spice manages stacked branches and coordinated pull requests.

> [!IMPORTANT]
> **All agent usage MUST include `--no-prompt`** to prevent blocking on interactive input.

## Installation Verification

```bash
which git-spice          # should return a path (e.g. /opt/homebrew/bin/git-spice)
git-spice --version      # expect 0.24.x or later
```

## One-Time Setup

### Per Repository (automated)

```bash
# Initialize git-spice with trunk = main
git-spice repo init --trunk main
```

### Authentication (human-only, interactive)

```bash
# Authenticate with GitHub — must be run by a human, cannot be automated
git-spice auth login
```

> [!NOTE]
> `git-spice auth login` is an interactive OAuth flow. Agents **cannot** perform this step. A human must run it once per environment before any `git-spice branch submit` operations.

## Project Git Config Defaults

These are set in `.git/config` by the initialization process. If missing in a fresh clone:

```bash
git config spice.submit.navigationComment true
git config spice.submit.navigationComment.downstack true
git config spice.log.all true
git config spice.log.crStatus true
```

> [!NOTE]
> `spice.submit.label` is **not** set as a project default. Pass `--label review-agent` explicitly when a PR is ready for review — not for draft PRs.

## Branch Naming Conventions

| Purpose | Branch name pattern |
|---|---|
| Feature / implementation | `issue-NNN` |
| Documentation | `docs/issue-NNN` |
| Refactoring | `refactor/issue-NNN` |
| Hotfix | `fix/issue-NNN` |

## Stack Base Determination

Before creating a branch, determine where to stack it:

```
If the issue has 'blocked-by' relationships pointing to another open issue:
  → Check out that blocking issue's branch
  → git-spice branch create issue-NNN

Otherwise (no blocking dependencies):
  → git-spice trunk                          # go to main/trunk
  → git-spice branch create issue-NNN
```

For the current milestone, non-trunk base is `feature/autonomous-pipeline`:

```bash
git checkout feature/autonomous-pipeline
git-spice branch create issue-NNN
```

## Core Commands

### Session Start

```bash
# Pull latest, clean merged branches, rebase stack — run at the start of every session
git-spice repo sync
```

### Branch Management

```bash
# Create a stacked branch (positioned on top of current branch)
git-spice branch create issue-NNN

# Create with a descriptive message (used in PR title)
git-spice branch create issue-NNN -m "feat: add authentication middleware"

# Navigate the stack
git-spice up           # move to the branch stacked on top of current
git-spice down         # move to the branch below current
git-spice trunk        # jump to trunk (main)

# Check stack state
git-spice log short                   # concise stack view
git-spice log long --json             # machine-readable full stack state
```

### Committing

```bash
# Commit and automatically restack upstack dependents
git-spice commit create -m "feat: implement thing"

# Amend the last commit and restack
git-spice commit amend -m "feat: implement thing (revised)"
```

### Submitting Pull Requests

```bash
# Submit current branch as PR — idempotent (creates or updates)
git-spice branch submit --fill --no-prompt

# Submit with the review-agent label (use when PR is ready for automated review)
git-spice branch submit --fill --no-prompt --label review-agent

# Submit entire stack as coordinated PRs
git-spice stack submit --fill --no-prompt

# Submit with explicit title and body (bypasses --fill inference)
git-spice branch submit --no-prompt \
  --title "feat(auth): add token refresh logic" \
  --body "Implements automatic token refresh on 401 responses."
```

### Syncing and Restacking

```bash
# After upstream merges: sync + rebase everything
git-spice repo sync --restack --no-prompt

# Rebase branches stacked above current branch
git-spice upstack restack

# Rebase only the current branch onto its parent
git-spice branch restack
```

## Conflict Resolution Protocol

If a restack hits a merge conflict:

```bash
# 1. Identify the conflicting files
git status

# 2. Resolve conflicts manually in each file

# 3. Stage resolved files
git add <resolved-file>

# 4. Continue the rebase
git-spice rebase continue
```

If the conflict is unresolvable:

```bash
# Abort and restore previous state
git-spice rebase abort

# Then: post to the engineering notification channel describing:
#   - Which branches conflict
#   - What the conflicting changes are
#   - What was attempted
# Wait for human guidance before retrying.
agent notify create \
  --title "Rebase conflict: needs human review" \
  --body "Conflict between <branch-A> and <branch-B>: <description>" \
  --channel engineering
```

## Labels Reference

Labels used in the autonomous pipeline workflow:

| Label | Applied by | Meaning |
|---|---|---|
| `review-agent` | Worker agent | PR is ready for automated review; triggers reviewer workflow |
| `merge-queue` | Reviewer agent | PR is approved; triggers conductor to merge |
| `needs-restack` | Reviewer agent | Branch is behind its parent; worker must restack before merge |
| `conductor-sync` | Any agent | Triggers an immediate conductor run for the associated issue |

## Common Workflows

### Submit a Single PR (ready for review)

```bash
git-spice branch submit --fill --no-prompt --label review-agent
```

### Submit a Stack (in progress, not yet ready)

```bash
git-spice stack submit --fill --no-prompt
```

### After a Dependency Is Merged

```bash
git-spice repo sync --restack --no-prompt
# Verify stack looks correct
git-spice log short
```

### Check if a Branch Has an Existing PR

```bash
git-spice log long --json | jq '.[] | {branch: .branch, pr: .pr}'
```

## Troubleshooting

### `not allowed to prompt for input`

Add `--no-prompt` to your command. All agent invocations must be non-interactive.

### `refs/spice/data` missing after fresh clone

git-spice state is stored in a git ref, not tracked files. Re-initialize:

```bash
git-spice repo init --trunk main
```

### Branch not tracked by git-spice

If you created a branch with `git checkout -b` instead of `git-spice branch create`, track it:

```bash
git-spice branch track --base <parent-branch>
```

### `gs` alias deprecation warning

The `gs` alias for `git-spice` is deprecated. Use `git-spice` directly, or set:

```bash
export GIT_SPICE_NO_GS_WARNING=1
```
