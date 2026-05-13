---
name: git-spice
description: Branch stacking and PR management with git-spice. Use when creating stacked branches, submitting PRs, syncing with upstream, or navigating a branch stack.
---

# git-spice

Reference skill for using git-spice (`git-spice`) in the agentd project.

> [!IMPORTANT]
> **All agent usage MUST include `--no-prompt`** to prevent blocking on interactive input.

## Common Workflows

```bash
# Create a new stacked branch
git-spice branch create issue-NNN -m "feat: description"

# Commit with auto-restack of dependents
git-spice commit create -m "feat(scope): description"

# Submit a PR
git-spice branch submit --no-prompt --label review-agent \
  --title "feat: <title>" \
  --body "<summary>\n\nCloses #N"

# Sync with upstream
git-spice repo sync

# Check stack state
git-spice log short
```

## Branch Naming

- Implementation: `issue-NNN`
- Documentation: `docs/issue-NNN`
- Refactoring: `refactor/issue-NNN`

## Notes

- Never use raw `git checkout -b` — use `git-spice branch create`
- Never use `gh pr create` directly — use `git-spice branch submit`
- If TLS errors occur, fall back to `git push origin <branch>` + `gh pr create --base <base> --head <branch>`
