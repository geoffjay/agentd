# agentd-index Service Documentation

The index service provides semantic code search over registered repositories.
It chunks source files using tree-sitter, generates embeddings via Ollama, and
stores them in LanceDB for fast approximate nearest-neighbour (ANN) search.
Optional BM25 full-text search via tantivy can be combined with vector search
using Reciprocal Rank Fusion (RRF).

## Base URL

```
http://127.0.0.1:17012
```

Port defaults to `17012`, configurable via `AGENTD_PORT`.

## Architecture

```
source files
     │
     ▼
  Chunker (tree-sitter)         ← semantic / hierarchical split
     │
     ▼
  Embedder (Ollama / OpenAI)    ← nomic-embed-code by default
     │
     ▼
  LanceDB vector store          ← ANN index for fast retrieval
     │
     ▼
  Search endpoint               ← vector / hybrid / keyword
```

### Search Modes

| Mode      | Algorithm              | Description                              |
|-----------|------------------------|------------------------------------------|
| `vector`  | ANN similarity         | Pure semantic search (default)           |
| `hybrid`  | Vector + BM25 via RRF  | Combines semantic and keyword relevance  |
| `keyword` | BM25 (tantivy)         | Exact term / identifier matching         |

## Environment Variables

| Variable                              | Default                              | Description                           |
|---------------------------------------|--------------------------------------|---------------------------------------|
| `AGENTD_PORT`                         | `17012`                              | HTTP listen port                      |
| `AGENTD_INDEX_EMBEDDING_PROVIDER`     | `ollama`                             | Embedding provider (`ollama`/`openai`)|
| `AGENTD_INDEX_EMBEDDING_MODEL`        | `manutic/nomic-embed-code`           | Embedding model name                  |
| `AGENTD_INDEX_EMBEDDING_ENDPOINT`     | `http://localhost:11434/v1`          | Ollama / OpenAI API endpoint          |
| `AGENTD_INDEX_LANCE_PATH`             | XDG data dir / `lancedb`             | LanceDB directory path                |
| `AGENTD_INDEX_LANCE_TABLE`            | `code_chunks`                        | LanceDB table name                    |
| `AGENTD_INDEX_WATCH_INTERVAL`         | `30`                                 | File watch poll interval (seconds)    |
| `AGENTD_INDEX_LANGUAGES`              | `rust,python,javascript,typescript`  | Comma-separated languages to index    |
| `AGENTD_INDEX_IGNORE_PATTERNS`        | `.git,target,node_modules,dist`      | Comma-separated patterns to skip      |
| `AGENTD_INDEX_SUMMARY_ENABLED`        | `false`                              | Enable LLM-generated chunk summaries  |
| `AGENTD_INDEX_SUMMARY_MODEL`          | `qwen2.5-coder:7b`                   | Ollama model for summaries            |
| `AGENTD_INDEX_RERANK_ENABLED`         | `false`                              | Enable cross-encoder reranking        |
| `AGENTD_INDEX_RERANK_MODEL`           | `qwen2.5-coder:7b`                   | Ollama model for reranking            |
| `AGENTD_INDEX_RERANK_CANDIDATES`      | `30`                                 | Candidate count before reranking      |
| `RUST_LOG`                            | `info`                               | Log level filter                      |

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "service": "agentd-index",
  "version": "0.2.0"
}
```

---

### Search

```
POST /search
```

Searches indexed code chunks using the configured search mode.

**Request body:**

```json
{
  "query": "authentication middleware",
  "search_mode": "vector",
  "repo_id": "agentd",
  "language": "rust",
  "file_pattern": "src/auth/**",
  "hierarchy_level": "symbol",
  "limit": 10
}
```

| Field            | Type     | Required | Default   | Description                                           |
|------------------|----------|----------|-----------|-------------------------------------------------------|
| `query`          | string   | yes      | —         | Natural-language or identifier query                  |
| `search_mode`    | string   | no       | `vector`  | `vector`, `hybrid`, or `keyword`                      |
| `repo_id`        | string   | no       | —         | Filter to a specific repository                       |
| `language`       | string   | no       | —         | Filter by language (`rust`, `python`, …)              |
| `file_pattern`   | string   | no       | —         | Glob pattern for file path filtering                  |
| `hierarchy_level`| string   | no       | —         | `symbol`, `file`, `directory`, or `repository`        |
| `limit`          | integer  | no       | `10`      | Max results (clamped to 1–100)                        |

**Response:**
```json
{
  "results": [
    {
      "id": "chunk_abc123_0",
      "file_path": "src/auth/middleware.rs",
      "language": "rust",
      "chunk_type": "function",
      "symbol_name": "authenticate_request",
      "start_line": 42,
      "end_line": 68,
      "content": "pub async fn authenticate_request(...) {",
      "summary": "Validates JWT tokens and attaches the user context.",
      "score": 0.92,
      "repo_id": "agentd"
    }
  ],
  "total": 1,
  "query_time_ms": 14
}
```

**Error responses:**
- `422 Unprocessable Entity` — empty or invalid query
- `500 Internal Server Error` — embedding or store failure

---

### Agentic Search (grep fallback)

```
POST /search/agentic
```

Grep-based search over raw source files.  Does not require the vector index —
useful for exact identifier lookup or when files have not yet been indexed.

**Request body:**

```json
{
  "query": "authenticate_request",
  "path": "crates/index/src",
  "file_pattern": "*.rs",
  "context_lines": 2,
  "limit": 20
}
```

| Field           | Type    | Required | Default | Description                                    |
|-----------------|---------|----------|---------|------------------------------------------------|
| `query`         | string  | yes      | —       | Basic regex passed to `grep`                   |
| `path`          | string  | no       | `.`     | Directory to search (relative to service cwd)  |
| `file_pattern`  | string  | no       | `*`     | `--include` glob for file filtering            |
| `context_lines` | integer | no       | `2`     | Lines of context before/after each match       |
| `limit`         | integer | no       | `20`    | Max matches (capped at 200)                    |

**Response:**
```json
{
  "matches": [
    {
      "file_path": "crates/index/src/api.rs",
      "line_number": 88,
      "content": "async fn authenticate_request(",
      "context_before": ["", "/// Authenticates an incoming request."],
      "context_after": ["    let token = req.headers().get(\"Authorization\");", "}"]
    }
  ],
  "total": 1,
  "query_time_ms": 12
}
```

---

### Register Repository

```
POST /repositories
```

Register a local repository for background indexing.

**Request body:**

```json
{
  "name": "agentd",
  "path": "/home/user/agentd"
}
```

**Response (201 Created):**
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "agentd",
  "path": "/home/user/agentd",
  "status": "pending",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "last_indexed": null,
  "error_message": null
}
```

---

### List Repositories

```
GET /repositories
```

**Response:**
```json
{
  "repositories": [
    {
      "id": "550e8400-...",
      "name": "agentd",
      "path": "/home/user/agentd",
      "status": "ready",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-01T01:00:00Z",
      "last_indexed": "2024-01-01T01:00:00Z"
    }
  ],
  "total": 1
}
```

---

### Get Repository

```
GET /repositories/{id}
```

Returns a single repository record.  `404 Not Found` if the ID is unknown.

---

### Delete Repository

```
DELETE /repositories/{id}
```

Removes a repository record.

- `204 No Content` on success
- `404 Not Found` if the ID is unknown

> **Note:** Indexed vector chunks for this repository are NOT automatically
> removed from LanceDB when a repository is deleted.

---

### Repository Status

```
GET /repositories/{id}/status
```

Returns the current indexing lifecycle status.

**Response:**
```json
{
  "id": "550e8400-...",
  "status": "ready",
  "last_indexed": "2024-01-01T01:00:00Z",
  "error_message": null
}
```

### Status Values

| Status     | Description                                               |
|------------|-----------------------------------------------------------|
| `pending`  | Registered; waiting for the background indexer to start  |
| `indexing` | Currently being indexed                                   |
| `ready`    | Successfully indexed; available for search                |
| `error`    | Last index run failed; see `error_message`                |

---

### Trigger Re-index

```
POST /repositories/{id}/reindex
```

Marks the repository as `pending` so the background watcher will schedule a
full re-index pass.

**Response (202 Accepted):**
```json
{ "status": "pending" }
```

- `404 Not Found` if the ID is unknown

---

## CLI Usage

The `agent index` subcommand provides a user-friendly interface to all
endpoints.  Service URL is read from `AGENTD_INDEX_SERVICE_URL` (default:
`http://localhost:17012`).

```bash
# Health
agent index health

# Repository management
agent index add-repo --name agentd --path /home/user/agentd
agent index list-repos
agent index status <repo-id>
agent index reindex <repo-id>
agent index remove-repo <repo-id>

# Code search
agent index search "authentication middleware"
agent index search "error handling" --mode hybrid --language rust
agent index search "ConnectionPool" --mode keyword --limit 5
agent index search "deploy handler" --file-pattern "src/api/**" --json
```

See `agent index --help` for all options.

## Claude Code Integration

The `agent-index` Claude Code skill (`.claude/skills/agent-index/`) enables
agents to search indexed code directly from conversations:

```
Use the agent-index skill to find the authentication middleware implementation.
```

## Running the Service

```bash
# Development
cargo run -p index
RUST_LOG=debug cargo run -p index

# With custom embedding endpoint
AGENTD_INDEX_EMBEDDING_ENDPOINT=http://my-ollama:11434/v1 cargo run -p index

# Production (via xtask)
cargo xtask start-services
```

## Metrics

The service exposes Prometheus metrics at `GET /metrics`.

| Metric                     | Description                              |
|----------------------------|------------------------------------------|
| `service_info`             | Service version and name gauge           |

Standard HTTP metrics (request count, latency, error rate) are emitted via
the `tower-http` tracing layer and captured by the Prometheus scraper.
