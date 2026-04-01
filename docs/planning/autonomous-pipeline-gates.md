# Autonomous Pipeline - Human Approval Gates

This document defines which operations in the agentd autonomous pipeline require
human approval, which are configurable, and which always run without intervention.
Gates are enforced through the orchestrator's tool policy system and through
`.claude/hooks/` PreToolUse hooks.

---

## Gate Categories

### ALWAYS HUMAN (cannot be automated)

These operations **must never** be performed by an agent without explicit human
instruction. No tool policy override exists.

| Operation | Reason |
|---|---|
| `git-spice auth login` | Interactive OAuth - one-time per environment; cannot be scripted |
| Changes to `.agentd/agents/*.yml` | Alters agent behavior and permissions for all future runs |
| Changes to `crates/orchestrator/src/` core | Risk of breaking the pipeline itself |
| Production deployments | Risk threshold requires human sign-off |
| Security-critical dependency changes | Flagged by auditor; may introduce CVEs |
| Merge conflict resolution (escalated by conductor) | Requires human judgment to avoid data loss |
| Adding new external service integrations | Architectural impact; may introduce new attack surface |
| Deletion of branches, issues, or milestones | Irreversible without git reflog; issues/milestones cannot be recovered |
| `git push --force` or `git reset --hard` to trunk branches | Destructive history rewrite |

> **Enforcement:** The `.claude/hooks/destructive-protection.py` hook blocks
> destructive git/gh commands. Any change to `.agentd/agents/` or
> `crates/orchestrator/src/` must go through a human-reviewed PR and cannot be
> committed directly by an agent running autonomously.

---

### CONFIGURABLE GATES (default shown; can be adjusted via tool policies)

These operations are safe to automate in most cases but may require human
oversight depending on environment or risk tolerance. Defaults represent the
policy shipped in this repository.

| Gate | Default | Rationale |
|---|---|---|
| PR auto-merge (approved + CI green) | **allow** | Low risk when both conditions are met |
| Issue auto-close after branch merge | **allow** | Clean bookkeeping; reversible |
| Planner creating issues autonomously | **allow** | Issues are soft state; easy to close |
| Applying labels to issues/PRs | **allow** | Reversible; purely metadata |
| New `Cargo.toml` dependency additions | **ask** | Introduces new supply-chain risk |
| Closing issues marked `security` | **ask** | Human should verify fix before closing |
| PR auto-merge when CI is flaky (retry > 2) | **ask** | Flakiness may hide real failures |
| Removing the `needs-rework` label | **allow** | Conductor re-dispatches; worker acts |

**Changing a gate:** Edit the `tool_policy` block in the relevant agent YAML
(`.agentd/agents/<agent>.yml`) and re-apply with `agent apply .agentd/`.

---

### ALWAYS AUTONOMOUS (no human approval needed)

These operations are safe and reversible enough that no approval gate is needed.

| Operation |
|---|
| Branch creation, switching, and deletion via git-spice |
| PR submission, updates, and stack navigation |
| Code review comments and requested changes |
| Test writing and documentation updates within `docs/` |
| Memory read/write operations (`agent memory *`) |
| Communication channel messages (`agent communicate *`) |
| Reading any file in the repository |
| Running `cargo build`, `cargo test`, `cargo fmt`, `cargo clippy` |
| Running `gh issue list`, `gh pr list`, and other read-only GitHub queries |
| Applying `needs-triage` label to newly filed issues |
| Posting status digests to the operations room |

---

## Tool Policy Configuration

Tool policies are declared in each agent's YAML under the `tool_policy` key.
They use glob/regex pattern matching against the command being executed.

### Example - Conductor

```yaml
tool_policy:
  # Allow squash merges (controlled by merge-ready gate check)
  - pattern: "gh pr merge * --squash *"
    mode: allow

  # Allow label changes (reversible metadata)
  - pattern: "gh * --add-label *"
    mode: allow
  - pattern: "gh * --remove-label *"
    mode: allow

  # Block all force pushes - always human
  - pattern: "git push --force*"
    mode: deny

  # Block deletions of issues or milestones - always human
  - pattern: "gh issue delete *"
    mode: deny
  - pattern: "gh api repos/*/milestones/* --method DELETE"
    mode: deny

  # Ask before new Cargo dependency additions
  - pattern: "cargo add *"
    mode: ask
```

### Example - Worker

```yaml
tool_policy:
  # Block modification of agent YAML files - always human
  - pattern: "* .agentd/agents/*"
    mode: deny

  # Ask before adding new Cargo dependencies
  - pattern: "cargo add *"
    mode: ask

  # Block force push
  - pattern: "git push --force*"
    mode: deny

  # Block direct issue or milestone deletion
  - pattern: "gh issue delete *"
    mode: deny
```

---

## Temporary Gate Override

To bypass a configurable gate for a single conductor run, use the
`conductor-sync` label with an override instruction in the PR body:

```bash
# Label the issue/PR
gh issue edit <number> --repo geoffjay/agentd --add-label "conductor-sync"

# The conductor reads the PR body for override instructions:
# OVERRIDE: allow cargo add serde_json (reason: pinning transitive dep)
```

The conductor checks the PR body for `OVERRIDE:` lines and relaxes the
corresponding policy for that run only. Overrides are logged to the operations
room for auditability.

---

## Escalation Path

When the conductor cannot proceed autonomously because a gate blocks it:

1. Conductor posts to the `engineering` room:
   ```
   ⛔ Gate blocked: <operation> requires human approval.
      PR/Issue: #<number>
      Reason: <why the gate fired>
      Action required: <what human must do to unblock>
   ```
2. Conductor leaves the PR labeled with the appropriate status (`needs-rework`,
   `needs-restack`, or `merge-ready` preserved).
3. Human resolves the gate condition and re-triggers via the appropriate label
   (`conductor-sync`, `merge-ready`, `agent`, etc.).

---

## References

- Tool policy DSL: `docs/public/tool-policies.md`
- Pipeline state machine: `docs/public/pipeline-state-machine.md`
- Destructive protection hook: `.claude/hooks/destructive-protection.py`
- Coordination hook: `.claude/hooks/check-coordination.py`
