---
name: agent-knowledge
description: Browse, read, and author per-project knowledgebase documents through the knowledge service CLI. Use for creating, updating, listing, and reading markdown documents stored against a project, and reconciling DB rows against disk.
---

# Agent Knowledge

Skill for interacting with the agentd **knowledge service** — a per-project store of
markdown documents, persisted as files on disk with SQLite metadata. Humans manage these
documents through the web UI; this skill lets agents do the same from the CLI.

All commands run through the core gateway (`AGENTD_CORE_SERVICE_URL`, default
`http://localhost:17000`) with bearer-token auth, so authenticate first:

```bash
agent auth login        # human-only, interactive — run once per machine
```

Every command is scoped to a **project UUID**. Discover available projects with:

```bash
agent project list
```

> Agents working inside a project run should already know their project ID (it is part of
> the agent's context). When in doubt, list projects and match on name.

## Listing Documents

```bash
# List all documents for a project (paginated, default 50)
agent knowledge list <project_id>

# Filter to a folder prefix
agent knowledge list <project_id> --prefix docs/

# Paginate
agent knowledge list <project_id> --limit 20 --offset 40

# Machine-readable output
agent knowledge list <project_id> --json
```

## Reading Documents

```bash
# Metadata only (id, path, title, size, timestamps)
agent knowledge get <project_id> <doc_id>

# Metadata + full markdown body
agent knowledge content <project_id> <doc_id>

# Hierarchical folder/file view of the whole project
agent knowledge tree <project_id>
```

## Creating Documents

`rel_path` must end in `.md`, use forward slashes for folders, and contain no `..` or
absolute path segments. Title defaults to the filename stem when omitted.

```bash
# Inline content
agent knowledge create <project_id> readme.md --content "# Hello"

# From a local file, with an explicit title
agent knowledge create <project_id> docs/api.md \
  --from-file ./api.md \
  --title "API Reference"
```

`--content` and `--from-file` are mutually exclusive.

## Updating Documents

```bash
# Replace the body inline
agent knowledge update <project_id> <doc_id> --content "# Updated"

# Replace the body from a file
agent knowledge update <project_id> <doc_id> --from-file ./updated.md

# Rename / retitle without touching the body
agent knowledge update <project_id> <doc_id> --title "New Title"
```

### Safe concurrent edits

To avoid clobbering a change made since you last read the document, pass the
`updated_at` timestamp from a prior `get`/`content` call. The update is rejected if the
document changed in the meantime — re-read and retry.

```bash
# 1. Read and note updated_at
agent knowledge get <project_id> <doc_id> --json   # -> "updated_at": "2026-06-17T12:34:56Z"

# 2. Update only if unchanged since then
agent knowledge update <project_id> <doc_id> \
  --from-file ./updated.md \
  --expected-updated-at 2026-06-17T12:34:56Z
```

## Deleting Documents

```bash
# Delete a single document (removes the DB row and the file on disk)
agent knowledge delete <project_id> <doc_id>

# Bulk-delete every document for a project (irreversible — requires --yes)
agent knowledge gc <project_id> --yes
```

## Reconciliation (doctor)

Detects and optionally repairs divergence between DB rows and files on disk —
**missing files** (DB rows whose file is gone) and **orphaned files** (files with no row).

```bash
# Read-only report
agent knowledge doctor <project_id>

# Repair: delete stale rows and orphaned files
agent knowledge doctor <project_id> --fix
```

## Health Check

```bash
agent knowledge health
```

## MCP equivalents

When running as an agent with the agentd MCP server connected, the same operations are
available as tools (no CLI needed): `knowledge_list_documents`, `knowledge_read_document`,
`knowledge_create_document`, `knowledge_update_document`, `knowledge_delete_document`, and
`knowledge_get_tree`. Prefer these during autonomous execution; use the CLI for
interactive/human use, bulk `gc`, and `doctor` reconciliation.
