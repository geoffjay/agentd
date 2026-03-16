# Migration: `source_config` → `trigger_config`

Phase 1 of the workflow refactor (#326) introduced the `TriggerStrategy` trait and renamed the workflow configuration fields to reflect the new abstraction. This page documents what changed and how to update existing installations and integrations.

---

## What Changed

| Before (≤ Phase 0) | After (Phase 1+) |
|--------------------|-----------------|
| `source_config` field in JSON | `trigger_config` |
| `source_type` column in DB | `trigger_type` |
| `TaskSourceConfig` Rust type | `TriggerConfig` |

The semantics are identical — only the names changed.

---

## JSON Payload Changes

### Create Workflow Request

=== "New (trigger_config)"

    ```json
    {
      "name": "issue-worker",
      "agent_id": "550e8400-e29b-41d4-a716-446655440000",
      "trigger_config": {
        "type": "github_issues",
        "owner": "myorg",
        "repo": "myrepo",
        "labels": ["agent"],
        "state": "open"
      },
      "prompt_template": "Work on issue #{{source_id}}: {{title}}\n\n{{body}}",
      "poll_interval_secs": 60,
      "enabled": true
    }
    ```

=== "Old (source_config) — still accepted"

    ```json
    {
      "name": "issue-worker",
      "agent_id": "550e8400-e29b-41d4-a716-446655440000",
      "source_config": {
        "type": "github_issues",
        "owner": "myorg",
        "repo": "myrepo",
        "labels": ["agent"],
        "state": "open"
      },
      "prompt_template": "Work on issue #{{source_id}}: {{title}}\n\n{{body}}",
      "poll_interval_secs": 60,
      "enabled": true
    }
    ```

### Workflow Response

The response from `GET /workflows/{id}` and `POST /workflows` now uses `trigger_config`:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "issue-worker",
  "agent_id": "550e8400-e29b-41d4-a716-446655440000",
  "trigger_config": {
    "type": "github_issues",
    "owner": "myorg",
    "repo": "myrepo",
    "labels": ["agent"],
    "state": "open"
  },
  "prompt_template": "Work on issue #{{source_id}}: {{title}}\n\n{{body}}",
  "poll_interval_secs": 60,
  "enabled": true,
  "tool_policy": { "mode": "auto" },
  "created_at": "2026-03-14T10:00:00Z",
  "updated_at": "2026-03-14T10:00:00Z"
}
```

---

## Backwards Compatibility

The Rust structs for `CreateWorkflowRequest`, `WorkflowResponse`, and `WorkflowConfig` all carry a `#[serde(alias = "source_config")]` attribute:

```rust
#[serde(alias = "source_config")]
pub trigger_config: TriggerConfig,
```

This means:

- **Existing API clients sending `source_config`** continue to work without any changes.
- **Existing stored workflow JSON** (e.g. in `.agentd/` template files) that uses `source_config` is still deserialised correctly.
- **Responses** always use `trigger_config` — if your client parses response bodies by field name, update it.

---

## Database Migration

Migration `m20260316_000007_rename_trigger_columns` runs automatically when the orchestrator starts. It renames the two affected columns in the `workflows` table:

| Old column | New column |
|------------|------------|
| `source_type` | `trigger_type` |
| `source_config` | `trigger_config` |

**SQLite note:** SQLite before version 3.25.0 does not support `ALTER TABLE … RENAME COLUMN`. The migration uses a copy-to-new-table approach:

1. Create `workflows_new` with the renamed columns.
2. Copy all rows from `workflows`, mapping `source_type` → `trigger_type` and `source_config` → `trigger_config`.
3. Drop `workflows`.
4. Rename `workflows_new` → `workflows`.

### Verifying the migration ran

```bash
sqlite3 ~/Library/Application\ Support/agentd-orchestrator/orchestrator.db \
  ".schema workflows"
```

The output should show `trigger_type` and `trigger_config` (not `source_type` / `source_config`).

### Manual rollback

The migration includes a `DOWN` path that reverses the rename. To trigger it you would need to roll back via the SeaORM migration CLI (`sea-orm-cli migrate down`) — this is not exposed as a regular `agentd` command and should only be used during development.

!!! warning
    Rolling back the migration on an installation that has already upgraded will cause the orchestrator to fail to start until the migration is re-applied.

---

## YAML Template Files

If you have `.agentd/workflows/*.yml` template files using `source_config`, they are still accepted. To adopt the new naming, update them:

=== "New"

    ```yaml
    name: issue-worker
    agent: worker
    trigger_config:
      type: github_issues
      owner: myorg
      repo: myrepo
      labels:
        - agent
    prompt_template: |
      Work on issue #{{source_id}}: {{title}}

      {{body}}
    poll_interval_secs: 60
    ```

=== "Old (still works)"

    ```yaml
    name: issue-worker
    agent: worker
    source_config:
      type: github_issues
      owner: myorg
      repo: myrepo
      labels:
        - agent
    prompt_template: |
      Work on issue #{{source_id}}: {{title}}

      {{body}}
    poll_interval_secs: 60
    ```

---

## CLI

The CLI `create-workflow` command already uses the new naming — it passes `trigger_config` in the JSON body it sends to the orchestrator. No CLI changes are required.

```bash
agent orchestrator create-workflow \
  --name issue-worker \
  --agent-name worker \
  --trigger-type github-issues \
  --owner myorg \
  --repo myrepo \
  --labels agent \
  --prompt-template "Work on #{{source_id}}: {{title}}\n\n{{body}}"
```

---

## Summary Checklist

- [x] API responses use `trigger_config` — update any response-parsing code
- [x] `source_config` in request bodies still works — no urgent client changes needed
- [x] DB migration runs automatically on orchestrator startup
- [x] `.agentd/` YAML templates using `source_config` still load correctly
- [ ] Optionally rename `source_config` → `trigger_config` in your YAML templates for consistency
