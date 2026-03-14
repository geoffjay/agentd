# agentd-memory API Documentation

The memory service provides persistent, semantically searchable storage for agent memories. It uses LanceDB as a vector store backend with OpenAI or Ollama embeddings for similarity search, and exposes a RESTful API for CRUD operations, visibility management, and semantic search.

## Base URL

```
http://127.0.0.1:17008
```

Port defaults to `17008`, configurable via the `AGENTD_PORT` environment variable.

## How It Works

1. A client stores a memory via `POST /memories` with natural-language content and metadata
2. The service generates an embedding vector from the content using the configured provider (OpenAI, Ollama, or none)
3. The memory and its embedding are persisted in the LanceDB vector store
4. Clients can retrieve memories by ID, list with filters, or perform semantic similarity search
5. Visibility controls (`public`, `shared`, `private`) determine which agents can access each memory

When the embedding provider is set to `"none"`, memories are stored without embeddings and semantic search is disabled.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17008` | HTTP listen port |
| `AGENTD_MEMORY_EMBEDDING_PROVIDER` | `none` | Embedding provider: `openai` or `none` |
| `AGENTD_MEMORY_EMBEDDING_MODEL` | `text-embedding-3-small` | Model name for the embedding provider |
| `AGENTD_MEMORY_EMBEDDING_API_KEY` | — | API key (required for remote OpenAI calls) |
| `AGENTD_MEMORY_EMBEDDING_ENDPOINT` | `https://api.openai.com/v1` | Base URL; set to `http://localhost:11434/v1` for Ollama |
| `AGENTD_MEMORY_LANCE_PATH` | XDG data dir / `agentd-memory/lancedb` | Filesystem path to the LanceDB directory |
| `AGENTD_MEMORY_LANCE_TABLE` | `memories` | LanceDB table name |
| `RUST_LOG` | `info` | Log level filter |

### Supported Embedding Models

**OpenAI:**
- `text-embedding-3-small` (1536 dimensions)
- `text-embedding-3-large` (3072 dimensions)
- `text-embedding-ada-002` (1536 dimensions)

**Ollama:**
- `nomic-embed-text` (768 dimensions)
- `mxbai-embed-large` (1024 dimensions)
- `all-minilm` (384 dimensions)
- `snowflake-arctic-embed` (1024 dimensions)

## Data Models

### Memory

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique ID (format: `mem_{timestamp}_{hex}`) |
| `content` | string | Natural-language content |
| `type` | string | Memory type: `information`, `question`, or `request` |
| `tags` | string[] | Tags for filtering |
| `created_by` | string | Identity of the actor who created the memory |
| `owner` | string? | Optional owner identity |
| `created_at` | datetime | Creation timestamp (ISO 8601) |
| `updated_at` | datetime | Last update timestamp (ISO 8601) |
| `visibility` | string | Visibility level: `public`, `shared`, or `private` |
| `shared_with` | string[] | Actor IDs the memory is shared with (for `shared` visibility) |
| `references` | string[] | IDs of related memories |

## Endpoints

### Health Check

```
GET /health
```

**Response:**
```json
{
  "status": "ok",
  "service": "agentd-memory",
  "version": "0.2.0",
  "details": {
    "vector_store": true
  }
}
```

If the vector store is unhealthy, returns HTTP 503 with `"status": "degraded"`.

**Example:**
```bash
curl -s http://127.0.0.1:17008/health | jq
```

---

### Prometheus Metrics

```
GET /metrics
```

Returns Prometheus-format metrics including `memories_created_total`, `memories_deleted_total`, and `memories_searched_total`.

**Example:**
```bash
curl -s http://127.0.0.1:17008/metrics
```

---

### Create Memory

Store a new memory record with an embedding vector for semantic search.

```
POST /memories
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `content` | string | yes | Natural-language content to store |
| `created_by` | string | yes | Identity of the actor creating this memory |
| `type` | string | no | Memory type: `information` (default), `question`, `request` |
| `tags` | string[] | no | Tags for filtering (default: `[]`) |
| `visibility` | string | no | Visibility level: `public` (default), `shared`, `private` |
| `shared_with` | string[] | no | Actor IDs to share with (default: `[]`) |
| `references` | string[] | no | IDs of related memories (default: `[]`) |

```json
{
  "content": "Paris is the capital of France.",
  "created_by": "agent-1",
  "type": "information",
  "tags": ["geography", "europe"],
  "visibility": "public"
}
```

**Response (201):**
```json
{
  "id": "mem_1705312801000_00000001",
  "content": "Paris is the capital of France.",
  "type": "information",
  "tags": ["geography", "europe"],
  "created_by": "agent-1",
  "owner": null,
  "created_at": "2024-01-15T10:00:01.000Z",
  "updated_at": "2024-01-15T10:00:01.000Z",
  "visibility": "public",
  "shared_with": [],
  "references": []
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Empty content or missing `created_by` |
| 500 | Embedding generation or storage failure |

**Example:**
```bash
curl -s -X POST http://127.0.0.1:17008/memories \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Paris is the capital of France.",
    "created_by": "agent-1",
    "type": "information",
    "tags": ["geography"]
  }' | jq
```

---

### List Memories

List memories with optional filters and pagination.

```
GET /memories
```

**Query Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `type` | string | — | Filter by memory type: `information`, `question`, `request` |
| `tag` | string | — | Filter by tag (comma-separated for multiple) |
| `created_by` | string | — | Filter by creator identity |
| `visibility` | string | — | Filter by visibility level |
| `limit` | integer | 50 | Max items per page (max: 200) |
| `offset` | integer | 0 | Pagination offset |

**Response (200):**
```json
{
  "items": [
    {
      "id": "mem_1705312801000_00000001",
      "content": "Paris is the capital of France.",
      "type": "information",
      "tags": ["geography"],
      "created_by": "agent-1",
      "visibility": "public",
      "shared_with": [],
      "references": [],
      "created_at": "2024-01-15T10:00:01.000Z",
      "updated_at": "2024-01-15T10:00:01.000Z"
    }
  ],
  "total": 1,
  "limit": 50,
  "offset": 0
}
```

**Examples:**
```bash
# List all memories
curl -s http://127.0.0.1:17008/memories | jq

# Filter by type and visibility
curl -s "http://127.0.0.1:17008/memories?type=question&visibility=public" | jq

# Paginate
curl -s "http://127.0.0.1:17008/memories?limit=10&offset=20" | jq

# Filter by tag and creator
curl -s "http://127.0.0.1:17008/memories?tag=devops&created_by=agent-1" | jq
```

---

### Get Memory

Retrieve a specific memory by ID.

```
GET /memories/:id
```

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Memory ID (e.g. `mem_1705312801000_00000001`) |

**Response (200):** Returns the full Memory object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 404 | Memory not found |

**Example:**
```bash
curl -s http://127.0.0.1:17008/memories/mem_1705312801000_00000001 | jq
```

---

### Delete Memory

Delete a memory by ID.

```
DELETE /memories/:id
```

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Memory ID |

**Response (200):**
```json
{
  "deleted": true
}
```

Returns `{"deleted": false}` if the memory was not found.

**Example:**
```bash
curl -s -X DELETE http://127.0.0.1:17008/memories/mem_1705312801000_00000001 | jq
```

---

### Semantic Search

Search memories using natural-language similarity. The query text is embedded and compared against stored memory vectors.

```
POST /memories/search
Content-Type: application/json
```

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | yes | Natural-language query for similarity search |
| `as_actor` | string | no | Actor performing the search (controls visibility filtering) |
| `type` | string | no | Filter by memory type |
| `tags` | string[] | no | Filter by tags |
| `limit` | integer | no | Maximum results (default: 10) |
| `from` | datetime | no | Only return memories created on or after this date (RFC 3339) |
| `to` | datetime | no | Only return memories created on or before this date (RFC 3339) |

```json
{
  "query": "capital of France",
  "as_actor": "agent-1",
  "type": "information",
  "tags": ["geography"],
  "limit": 5
}
```

**Response (200):**
```json
{
  "memories": [
    {
      "id": "mem_1705312801000_00000001",
      "content": "Paris is the capital of France.",
      "type": "information",
      "tags": ["geography"],
      "created_by": "agent-1",
      "visibility": "public",
      "shared_with": [],
      "references": [],
      "created_at": "2024-01-15T10:00:01.000Z",
      "updated_at": "2024-01-15T10:00:01.000Z"
    }
  ],
  "total": 1
}
```

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Empty query |
| 500 | Embedding or search failure |

**Example:**
```bash
curl -s -X POST http://127.0.0.1:17008/memories/search \
  -H "Content-Type: application/json" \
  -d '{"query": "capital of France", "limit": 5}' | jq
```

---

### Update Visibility

Update the visibility level and share list of a memory.

```
PUT /memories/:id/visibility
Content-Type: application/json
```

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | Memory ID |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `visibility` | string | yes | New visibility: `public`, `shared`, or `private` |
| `shared_with` | string[] | no | Actor IDs to share with (relevant for `shared` visibility) |
| `as_actor` | string | no | Actor making the change (enables ownership check) |

```json
{
  "visibility": "shared",
  "shared_with": ["agent-2", "agent-3"]
}
```

**Response (200):** Returns the updated Memory object.

**Errors:**

| Status | Condition |
|--------|-----------|
| 400 | Invalid visibility level, or `as_actor` is not the creator/owner |
| 404 | Memory not found |
| 500 | Storage failure |

**Example:**
```bash
curl -s -X PUT http://127.0.0.1:17008/memories/mem_1705312801000_00000001/visibility \
  -H "Content-Type: application/json" \
  -d '{
    "visibility": "shared",
    "shared_with": ["agent-2"]
  }' | jq
```

---

## CLI Commands

The `agent memory` command group provides a CLI interface to all memory service operations.

### Health Check

```bash
agent memory health
```

### Store a Memory

```bash
agent memory remember "Paris is the capital of France." \
  --created-by agent-1 \
  --type information \
  --tags geography,europe \
  --visibility public
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--created-by` | yes | — | Actor identity |
| `--type` | no | `information` | Memory type: `information`, `question`, `request` |
| `--tags` | no | — | Comma-separated tags |
| `--visibility` | no | `public` | Visibility: `public`, `shared`, `private` |
| `--share-with` | no | — | Comma-separated actor IDs (for `shared` visibility) |
| `--references` | no | — | Comma-separated related memory IDs |

### Recall a Memory

```bash
agent memory recall mem_1705312801000_00000001
```

### Semantic Search

```bash
agent memory search "capital of France" --limit 5
agent memory search "deployment steps" --as-actor agent-1 --type question --tags devops
agent memory search "recent events" --since 2024-01-01T00:00:00Z --until 2024-12-31T23:59:59Z
```

| Flag | Default | Description |
|------|---------|-------------|
| `--as-actor` | — | Actor performing the search (visibility filtering) |
| `--type` | — | Filter by memory type |
| `--tags` | — | Comma-separated tags to filter by |
| `--limit` | `10` | Maximum results |
| `--since` | — | Only memories created on or after this date (RFC 3339) |
| `--until` | — | Only memories created on or before this date (RFC 3339) |

### Delete a Memory

```bash
agent memory forget mem_1705312801000_00000001
```

### List Memories

```bash
agent memory list
agent memory list --type question --visibility public --limit 20
agent memory list --created-by agent-1 --tag auth --offset 10
```

| Flag | Default | Description |
|------|---------|-------------|
| `--type` | — | Filter by memory type |
| `--tag` | — | Filter by tag |
| `--created-by` | — | Filter by creator |
| `--visibility` | — | Filter by visibility level |
| `--limit` | `50` | Max items to return |
| `--offset` | `0` | Pagination offset |

### Update Visibility

```bash
agent memory visibility mem_1705312801000_00000001 shared --share-with agent-2,agent-3
agent memory visibility mem_1705312801000_00000001 private
```

All CLI commands support a `--json` flag for machine-readable JSON output.

## Running the Service

```bash
# Development (no embeddings)
cargo run -p memory

# With OpenAI embeddings
AGENTD_MEMORY_EMBEDDING_PROVIDER=openai \
AGENTD_MEMORY_EMBEDDING_API_KEY=sk-... \
cargo run -p memory

# With Ollama (local embeddings)
AGENTD_MEMORY_EMBEDDING_PROVIDER=openai \
AGENTD_MEMORY_EMBEDDING_MODEL=nomic-embed-text \
AGENTD_MEMORY_EMBEDDING_ENDPOINT=http://localhost:11434/v1 \
cargo run -p memory

# With debug logging
RUST_LOG=debug cargo run -p memory

# Custom storage path and port
AGENTD_PORT=8080 \
AGENTD_MEMORY_LANCE_PATH=/tmp/my-memories \
cargo run -p memory
```

### Data Storage

LanceDB stores data on the local filesystem. The default path is platform-specific:

- **Linux**: `~/.local/share/agentd-memory/lancedb`
- **macOS**: `~/Library/Application Support/agentd-memory/lancedb`

Override with the `AGENTD_MEMORY_LANCE_PATH` environment variable.
