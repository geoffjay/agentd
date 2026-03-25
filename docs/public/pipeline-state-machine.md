# Pipeline State Machine

The agentd autonomous pipeline is driven entirely by GitHub labels. Each label
either encodes the current state of an issue/PR or triggers dispatch to a
specialized agent. The conductor agent reads and transitions these labels to
drive the end-to-end lifecycle.

## Label Color Convention

| Color family | Hex prefix | Purpose |
|---|---|---|
| Blue | `0ea5e9`, `2563eb`, `1d4ed8` | Pipeline states — where an issue is right now |
| Amber / yellow | `f59e0b`, `e4e669`, `fbbf24` | Status warnings — something needs attention |
| Green | `4ade80`, `22c55e`, `16a34a`, `5bdc3b`, `d1f8a4` | Agent dispatch — triggers a specialized agent |
| Orange / indigo | `f97316`, `818cf8`, `6366f1` | Specialist dispatch — security, research, orchestration |
| Red | `e11d48`, `d93f0b` | Blocking states — must be resolved before proceeding |

## State Diagram

```
Issue Created
    │
    ▼
[needs-triage] ──► Planner/Triage Agent
    │
    ▼
[triaged]
    │
    ├──► [enrich-agent] ──► Enricher Agent (optional, adds context)
    │         │
    │         ▼
    │    [triaged] (enriched)
    │
    ▼
[agent] ──► Worker Agent implements
    │
    ▼
PR Created + [review-agent] ──► Reviewer Agent
    │
    ├── Approved ──► [merge-ready] ──► Conductor merges
    │                     │
    │                     ▼
    │                  Merged
    │                     │
    │                     └──► [docs-agent] (optional, if API changed)
    │
    └── Changes Requested ──► [needs-rework] ──► Worker re-dispatched
              │
              ▼
           Worker fixes ──► [review-agent] again
```

## State Reference

### Pipeline States (blue labels)

| Label | Applied by | Meaning |
|-------|-----------|---------|
| `needs-triage` | Human / automation | New issue awaiting triage |
| `triaged` | Planner / triage agent | Issue scope and labels assigned |
| `merge-ready` | Reviewer agent | PR approved + CI passing; ready for merge |
| `merge-queue` | Conductor | Acknowledged by conductor, actively queued |

### Status / Warning Labels (amber + red labels)

| Label | Applied by | Meaning |
|-------|-----------|---------|
| `needs-rework` | Reviewer agent | PR has change requests; worker must address |
| `needs-restack` | Conductor | Branch is behind parent branch; must rebase |
| `needs-refinement` | Human / planner | Issue lacks enough detail to implement |
| `needs-tests` | Reviewer / tester agent | Missing test coverage |

### Agent Dispatch Labels (green + specialist labels)

Applying one of these labels causes the corresponding workflow to poll it and
dispatch the matching agent. The agent removes the label when done.

| Label | Workflow | Agent | Trigger target |
|-------|----------|-------|---------------|
| `work-agent` | issue-worker | worker | Issue |
| `review-agent` | pull-request-reviewer | reviewer | Pull request |
| `plan-agent` | plan-worker | planner | Issue |
| `docs-agent` | docs-worker | documenter | Issue / PR |
| `enrich-agent` | enrichment-worker | enricher | Issue |
| `test-agent` | test-worker | tester | Issue / PR |
| `refactor-agent` | refactor-worker | refactor agent | Issue / PR |
| `research-agent` | research-worker | research agent | Issue |
| `security-agent` | security-worker | security agent | Issue / PR |
| `conductor-sync` | conductor-sync workflow | conductor | Manual trigger |

## Full Lifecycle Example

```
1.  Issue #42 created
2.  Human applies `needs-triage`
3.  Planner agent triages → removes `needs-triage`, applies `triaged` + `agent`
4.  Issue-worker picks up `agent` → worker implements, opens PR, applies `review-agent`
5.  Reviewer agent reviews PR:
    a.  Approves → applies `merge-ready`, removes `review-agent`
    b.  Requests changes → applies `needs-rework`, removes `review-agent`
        → Worker addresses feedback, removes `needs-rework`, re-applies `review-agent`
        → Loop back to step 5
6.  Conductor sees `merge-ready`, CI green, no blockers → merges PR
7.  If API changed: conductor applies `docs-agent` → documenter writes/updates docs
8.  Issue closed, branch deleted
```

## Deduplication

The orchestrator's scheduler tracks every `(workflow_id, source_id)` pair in a
dispatch record store. Re-applying a label to an already-dispatched issue does
not trigger a second dispatch — the `storage.is_dispatched()` check in
`crates/orchestrator/src/scheduler/runner.rs` prevents duplicates.

To re-trigger an agent on the same issue/PR, the label must first be removed
and then re-added (which creates a new event that clears the dedup record only
when the previous dispatch is marked complete or failed).

## Related Files

- `.agentd/agents/conductor.yml` — Conductor agent system prompt and dispatch logic
- `.agentd/agents/*.yml` — Individual agent definitions
- `.agentd/workflows/*.yml` — Workflow definitions (one per dispatch label)
- `.github/labels.yml` — Authoritative label configuration for this repository
- `crates/orchestrator/src/scheduler/runner.rs` — Dispatch dedup logic
