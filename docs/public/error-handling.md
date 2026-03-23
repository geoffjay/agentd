# Error Handling and Logging Conventions

This guide documents the standard patterns for error handling and logging across all agentd service crates.

## Error Handling

### Principles

1. **Shared `ApiError` in `agentd-common`** — the common crate provides a single `ApiError` enum that most services re-export directly
2. **Use `anyhow` for internal propagation** — internal helpers and storage operations return `anyhow::Result`
3. **Error responses are JSON** — all errors return `{"error": "message"}` format
4. **Domain-specific services define their own error type** — services with unique error cases (e.g., `ask`) define a custom `ApiError` instead

### Shared ApiError

The canonical error type lives in [`crates/common/src/error.rs`](../../crates/common/src/error.rs):

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// HTTP 404
    #[error("not found")]
    NotFound,

    /// HTTP 401
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// HTTP 403
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// HTTP 400
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// HTTP 409
    #[error("conflict: {0}")]
    Conflict(String),

    /// HTTP 503
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    /// HTTP 500 — wraps any anyhow::Error via `?`
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
```

**Status code mapping:**

| Variant | HTTP Status |
|---------|-------------|
| `NotFound` | 404 Not Found |
| `Unauthorized(String)` | 401 Unauthorized |
| `Forbidden(String)` | 403 Forbidden |
| `InvalidInput(String)` | 400 Bad Request |
| `Conflict(String)` | 409 Conflict |
| `ServiceUnavailable(String)` | 503 Service Unavailable |
| `Internal(anyhow::Error)` | 500 Internal Server Error |

All variants produce a JSON body: `{"error": "<message>"}`.

### Usage Patterns

=== "Re-export (most services)"

    Services that don't need domain-specific error variants re-export the shared type directly at the bottom of their `api.rs`:

    ```rust
    // crates/notify/src/api.rs
    pub use agentd_common::error::ApiError;
    ```

    This is the pattern used by `notify`, `orchestrator`, `communicate`, and `memory`.

=== "Custom error type (domain services)"

    Services with domain-specific error cases define their own `ApiError` in a dedicated `error.rs`:

    ```rust
    // crates/ask/src/error.rs
    #[derive(Debug, thiserror::Error)]
    pub enum ApiError {
        #[error("question not found: {0}")]
        QuestionNotFound(String),          // 404

        #[error("question is no longer actionable: {0}")]
        QuestionNotActionable(String),     // 410

        #[error("tmux error: {0}")]
        TmuxError(#[from] TmuxError),      // 500

        #[error("notification error: {0}")]
        NotificationError(#[from] NotificationError), // 502

        #[error("internal error: {0}")]
        InternalError(String),             // 500
    }

    impl From<anyhow::Error> for ApiError {
        fn from(err: anyhow::Error) -> Self {
            ApiError::InternalError(err.to_string())
        }
    }
    ```

    This is the pattern used by `ask`, `hook`, and `monitor`.

### IntoResponse Implementation

`agentd_common::error::ApiError` implements `axum::response::IntoResponse`:

```rust
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::ServiceUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
```

### Error Propagation

Use `?` with `anyhow::Error` in handlers — it converts automatically via `#[from]`:

```rust
async fn get_agent(
    Path(id): Path<Uuid>,
    State(state): State<ApiState>,
) -> Result<Json<Agent>, ApiError> {
    let agent = state.manager.get_agent(&id).await?.ok_or(ApiError::NotFound)?;
    Ok(Json(agent))
}
```

For invalid input validation:

```rust
let status = body.status
    .parse::<AgentStatus>()
    .map_err(|e| ApiError::InvalidInput(e.to_string()))?;
```

For conflicting state:

```rust
if agent.status == AgentStatus::Running {
    return Err(ApiError::Conflict(format!("agent {} is already running", id)));
}
```

### Adding New Error Handling

**For services using the shared `ApiError`** (notify, orchestrator, communicate, memory):

The seven shared variants cover most cases. Use the most appropriate variant:

- `NotFound` — resource doesn't exist
- `InvalidInput(msg)` — bad request data, failed validation
- `Conflict(msg)` — state transition not allowed, duplicate resource
- `Unauthorized(msg)` — authentication failure
- `Forbidden(msg)` — authenticated but not permitted
- `Internal(anyhow_err)` — use `?` on any `anyhow::Result`

**For services with a custom `ApiError`** (ask, hook, monitor):

1. Add a variant to the crate's `ApiError` enum in `error.rs` with `#[error("...")]`
2. Choose an appropriate HTTP status code
3. Add the variant to the `IntoResponse` match arm
4. If wrapping another error type, add `#[from]` for automatic conversion

---

## Logging

### Server Initialization

All services call `agentd_common::server::init_tracing()` once in `main()` — defined in [`crates/common/src/server.rs`](../../crates/common/src/server.rs):

```rust
use agentd_common::server::init_tracing;

#[tokio::main]
async fn main() {
    init_tracing();
    // ...
}
```

### Environment Variables

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `RUST_LOG` | `trace`, `debug`, `info`, `warn`, `error` | `info` | Log level filter |
| `AGENTD_LOG_FORMAT` | `json`, (unset) | human-readable | Output format |

### JSON Logging

For production environments or log aggregation, enable structured JSON output:

```bash
AGENTD_LOG_FORMAT=json cargo run -p agentd-notify
```

Output format:
```json
{"timestamp":"2026-03-02T12:00:00.000Z","level":"INFO","fields":{"message":"Starting agentd-notify service..."},"target":"agentd_notify"}
```

### Request Tracing

All HTTP services use `agentd_common::server::trace_layer()` for request/response logging:

```rust
let app = Router::new()
    .route("/health", get(health_check))
    .layer(agentd_common::server::trace_layer());
```

This automatically logs every request:

```
2026-03-02T12:00:00.001Z  INFO request{method=GET uri=/health} started
2026-03-02T12:00:00.002Z  INFO request{method=GET uri=/health} completed status=200 latency=1ms
```

### CORS

Services that accept cross-origin requests use `agentd_common::server::cors_layer()`:

```rust
let app = Router::new()
    // ...
    .layer(agentd_common::server::cors_layer());
```

Allowed origins are configured via environment variable:

| Variable | Values | Default |
|----------|--------|---------|
| `AGENTD_CORS_ORIGINS` | Comma-separated origin list | `*` (any origin) |

Example for production:

```bash
AGENTD_CORS_ORIGINS=https://app.example.com,https://admin.example.com cargo run -p agentd-notify
```

Allowed methods: `GET, POST, PUT, DELETE, OPTIONS`
Allowed headers: `Content-Type`, `Authorization`, WebSocket upgrade headers

### Logging Levels

| Level | Use For |
|-------|---------|
| `error!` | Unrecoverable failures, data loss, service-breaking issues |
| `warn!` | Recoverable issues, degraded operation, retries |
| `info!` | Service lifecycle (start/stop), important state changes |
| `debug!` | Detailed operation traces, request/response bodies, internal state |
| `trace!` | Very verbose debugging, message-level protocol traces |

### Structured Fields

Use structured fields in log messages:

```rust
// Good — structured fields enable log filtering and querying
info!(agent_id = %id, status = %agent.status, "Agent state changed");

// Avoid — unstructured strings are harder to query
info!("Agent {} changed to {}", id, agent.status);
```

---

## Dependencies

All crates use workspace-level dependency versions from the root `Cargo.toml`:

```toml
[workspace.dependencies]
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

Service crates also include:

```toml
tower-http = { version = "0.6", features = ["trace", "cors"] }
agentd-common = { path = "../common" }
```
