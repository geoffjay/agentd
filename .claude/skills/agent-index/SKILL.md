---
name: agent-index
description: Register repositories, trigger indexing, and search indexed code through the agentd-index service. Use for semantic code search, repository management, and checking indexing status. The index service runs on port 17012.
---

# Agent Index

Skill for interacting with the agentd-index service — a semantic code search engine that indexes source repositories using LanceDB vector storage and BM25 full-text search.

## Registering Repositories

```bash
# Register a repository for indexing
agent index add-repo --name agentd --path /home/user/agentd

# Register with explicit absolute path
agent index add-repo --name my-project --path /projects/my-project

# JSON output
agent index add-repo --name my-project --path /projects/my-project --json
```

## Listing and Managing Repositories

```bash
# List all registered repositories
agent index list-repos

# JSON output
agent index list-repos --json

# Check indexing status for a specific repository
agent index status <repo-id>

# Remove a repository from the registry
agent index remove-repo <repo-id>
```

## Triggering Re-indexing

```bash
# Trigger a full re-index for a repository
agent index reindex <repo-id>

# The service will set status to "pending" and re-index in the background
agent index status <repo-id>   # poll until status is "ready"
```

## Searching Code

Semantic vector search (default):

```bash
# Basic semantic search
agent index search "authentication middleware"

# Limit results
agent index search "database connection pool" --limit 5

# Filter by language
agent index search "error handling patterns" --language rust

# Filter by file glob pattern
agent index search "HTTP handler" --file-pattern "src/api/**"

# Filter by hierarchy level
agent index search "auth function" --hierarchy symbol

# Filter by repository
agent index search "deploy workflow" --repo agentd

# JSON output for programmatic use
agent index search "parse config" --json
```

Hybrid search (vector + BM25):

```bash
# Hybrid search combines semantic and keyword matching
agent index search "authenticate_request" --mode hybrid

# Keyword-only BM25 search (exact term matching)
agent index search "impl CodeStore" --mode keyword
```

### Search Modes

| Mode      | Description                                   | Best For                          |
|-----------|-----------------------------------------------|-----------------------------------|
| `vector`  | Semantic similarity (default)                 | Natural language queries          |
| `hybrid`  | Vector + BM25 combined via RRF                | Mixed natural + keyword queries   |
| `keyword` | BM25 full-text only                           | Exact identifiers, symbol names   |

### Hierarchy Levels

| Level        | Matches                                         |
|--------------|-------------------------------------------------|
| `symbol`     | Functions, structs, methods, classes            |
| `file`       | Whole-file chunks                               |
| `directory`  | Directory-level summaries                       |
| `repository` | Repository-level overviews                      |

## Health Check

```bash
# Check if the index service is running
agent index health
```

## Service Configuration

| Environment Variable         | Default                     | Description                  |
|------------------------------|-----------------------------|------------------------------|
| `AGENTD_INDEX_SERVICE_URL`   | `http://localhost:17012`    | Index service base URL       |

## Repository Status Lifecycle

```
Pending → Indexing → Ready
                  ↘ Error
```

- **pending** — Registered but not yet indexed (or queued for re-index)
- **indexing** — Currently being indexed in the background
- **ready** — Successfully indexed and available for search
- **error** — Indexing failed; check status for error details

## Example Workflow

```bash
# 1. Register a repository
agent index add-repo --name my-service --path /home/user/my-service
# → Returns: id, name, path, status: pending

# 2. Check when indexing completes
agent index status <repo-id>
# → status: indexing ... then ready

# 3. Search the indexed code
agent index search "connection pool implementation" --language rust --limit 10

# 4. Use hybrid mode for identifier search
agent index search "ConnectionPool" --mode hybrid

# 5. Re-index after large changes
agent index reindex <repo-id>
```
