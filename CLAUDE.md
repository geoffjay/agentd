# agentd — Claude Code Guide

This file provides guidance for Claude Code and AI agents working on the agentd repository.

## Repository Overview

agentd is a Rust workspace containing multiple services. See `README.md` for the full overview.

## Branch Strategy

All feature work branches off `feature/autonomous-pipeline` and PRs back into it — **never directly to `main`**.

```bash
git checkout feature/autonomous-pipeline
git checkout -b issue-<number>
# ... implement changes ...
git-spice branch submit
```

## git-spice (Branch Stacking)

git-spice (`git-spice`) is **required** for branch management in this project. It manages stacked branches, pull request navigation comments, and change request status tracking.

### Installation

```bash
# macOS
brew install abhinav/tap/git-spice

# Verify
git-spice --version
```

### One-Time Setup (per clone)

```bash
# Initialize git-spice (trunk = main)
git-spice repo init --trunk main
```

> [!IMPORTANT]
> **Human-only step:** `git-spice auth login` is an interactive OAuth flow that requires a human to complete. Run this once before any `git-spice branch submit` operations. Agents cannot perform this step.
>
> ```bash
> git-spice auth login
> ```

### Project Config Defaults

These are stored in `.git/config` and should be present after initialization:

```gitconfig
[spice]
    submit.navigationComment = true
    submit.navigationComment.downstack = true
    log.all = true
    log.crStatus = true
```

If these are missing in a fresh clone, restore them with:

```bash
git config spice.submit.navigationComment true
git config spice.submit.navigationComment.downstack true
git config spice.log.all true
git config spice.log.crStatus true
```

### Common Workflows

```bash
# Check stacked branch status
git-spice log short

# Create a new branch on top of current
git-spice branch create <branch-name>

# Submit branch as a pull request
git-spice branch submit

# Submit with the review-agent label (when ready for review)
git-spice branch submit --label review-agent

# Sync with upstream changes
git-spice branch sync

# Restack after upstream changes
git-spice branch restack
```

> [!NOTE]
> Do **not** set `spice.submit.label = review-agent` as a project default. Agents should pass `--label review-agent` explicitly only when the PR is ready for review (e.g., not for draft PRs).

### Verifying git-spice State

```bash
# Verify the spice data ref exists
git log --oneline refs/spice/data | head -5

# Check clean log state against main
git-spice log short
```

### References

- [git-spice documentation](https://abhinav.github.io/git-spice/)
- git-spice skill: use the `agent-memory` skill for agent workflows

## Development Commands

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy

# Run a specific service
cargo run -p agentd-orchestrator
```

## Human Approval Gates

Certain operations **must never** be performed by an agent without explicit
human instruction. The full gate reference is in
`docs/planning/autonomous-pipeline-gates.md`.

**Always-human operations (abridged):**

- `git-spice auth login` — interactive OAuth; cannot be scripted
- Changes to `.agentd/agents/*.yml` — alters agent behavior for all future runs
- Changes to `crates/orchestrator/src/` core — risk of breaking the pipeline
- Production deployments
- Adding new external service integrations
- Deletion of branches, issues, or milestones
- `git push --force` or `git reset --hard` to trunk branches

> [!CAUTION]
> Agents must **not** bypass these gates. The `.claude/hooks/destructive-protection.py`
> hook enforces this at the PreToolUse level.

## Code Conventions

- Follow existing Rust idioms and patterns in each crate
- Use `anyhow` for error handling in binaries, `thiserror` for library errors
- Write tests for new functionality
- Keep changes focused on the issue scope — don't refactor unrelated code
- Run `cargo fmt` and `cargo clippy` before committing

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **agentd** (13778 symbols, 28516 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/agentd/context` | Codebase overview, check index freshness |
| `gitnexus://repo/agentd/clusters` | All functional areas |
| `gitnexus://repo/agentd/processes` | All execution flows |
| `gitnexus://repo/agentd/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
