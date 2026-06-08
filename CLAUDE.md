# agentd — Claude Code Guide

This file provides guidance for Claude Code and AI agents working on the agentd repository.

## Repository Overview

agentd is a Rust workspace containing multiple services. See `README.md` for the full overview.

## Removed: Code Index Service

The `index` crate (`agentd-index`, a semantic + keyword code search service on port 17012)
and all of its supporting code — CLI commands, config, UI feature, packaging, and docs — were
**removed** as an experiment whose compilation, installation, and maintenance overhead outweighed
its benefit. If future work calls for re-adding indexing or semantic code search, recover the prior
implementation from git rather than building it from scratch:

- The last commit that contained the index crate was `4b71527e` (release v0.4.2).
- To find the exact removal commit later, run `git log --oneline --diff-filter=D -- crates/index`.
- Inspect the old code with `git show 4b71527e:crates/index/Cargo.toml` (or check out the tree:
  `git checkout 4b71527e -- crates/index`), which captures the dependencies (tree-sitter, lancedb,
  tantivy, arrow, etc.), chunking pipeline, embedding store, and API surface that existed before.

## Branch Strategy

All feature work branches off of a feature branch prefixed by `feature/` with the scope appended as
a related name. All PRs should be merged back into it — **never directly to `main`**.

```bash
git checkout feature/scope-of-work
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
