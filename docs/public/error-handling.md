# Error Handling and Logging Conventions

This guide documents the standard patterns for error handling and logging across all agentd service crates.

## Error Handling

### Principles

1. **Use `thiserror` for domain errors** — all service crates define their `ApiError` enum with `#[derive(Debug, thiserror::Error)]`
2. **Use `anyhow` for internal propagation** — internal helpers and storage operations return `anyhow::Result`
3. **Map errors to HTTP status codes** — every `ApiError` variant has a defined HTTP status via `IntoResponse`
4. **Error responses are JSON** — all errors return `{"error": "message"}` format

### Standard ApiError Pattern

Every HTTP service crate defines an `ApiError` enum in its `api.rs` with this pattern:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Maps to HTTP 500
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),

    /// Maps to HTTP 404
    #[error("not found: {0}")]
    NotFound(String),

    /// Maps to HTTP 400
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
```

Additional domain-specific variants can be added as needed:

| Crate | Extra Variants | Status Code |
|-------|---------------|-------------|
| ask | `QuestionNotFound` | 404 |
| ask | `QuestionNotActionable` | 410 |
| ask | `TmuxError` | 500 |
| ask | `NotificationError` | 502 |
| orchestrator | `AgentNotRunning` | 409 |

### IntoResponse Implementation

All `ApiError` enums implement `axum::response::IntoResponse`:

```rust
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            // ... other variants
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
```

### Error Propagation

- Use `?` operator with `anyhow::Error` in handlers — it automatically converts via `#[from]`
- For cross-service errors, wrap the upstream error with context:

```rust
state.notification_client
    .create_notification(request)
    .await
    .map_err(|e| ApiError::NotificationError(e))?;
```

### Adding New Error Types

When adding a new error type to a service:

1. Add a variant to the crate's `ApiError` enum with `#[error("...")]`
2. Choose an appropriate HTTP status code
3. Add the variant to the `IntoResponse` match
4. If wrapping another error, use `#[from]` for automatic conversion

## Logging

### Environment Variables

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `RUST_LOG` | `trace`, `debug`, `info`, `warn`, `error` | `info` | Log level filter |
| `AGENTD_LOG_FORMAT` | `json`, (unset) | (unset = human-readable) | Output format |

### Standard Initialization

All services initialize logging the same way in `main()`:

```rust
let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

if std::env::var("AGENTD_LOG_FORMAT").as_deref() == Ok("json") {
    tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
} else {
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}
```

### JSON Logging

For production environments or log aggregation pipelines, enable structured JSON output:

```bash
AGENTD_LOG_FORMAT=json cargo run -p agentd-notify
```

Output format:
```json
{"timestamp":"2026-03-02T12:00:00.000Z","level":"INFO","fields":{"message":"Starting agentd-notify service..."},"target":"agentd_notify"}
```

### Request Tracing

All HTTP services include `tower-http` TraceLayer middleware that automatically logs:

- Request method and path
- Response status code
- Request duration

```
2026-03-02T12:00:00.000Z  INFO request{method=GET uri=/health} started
2026-03-02T12:00:00.001Z  INFO request{method=GET uri=/health} completed status=200 latency=1ms
```

### Logging Levels

Follow these conventions:

| Level | Use For |
|-------|---------|
| `error!` | Unrecoverable failures, data loss, service-breaking issues |
| `warn!` | Recoverable issues, degraded operation, retries |
| `info!` | Service lifecycle (start/stop), request summaries, important state changes |
| `debug!` | Detailed operation traces, request/response bodies, internal state |
| `trace!` | Very verbose debugging, message-level protocol traces |

### Structured Fields

Use structured fields in log messages:

```rust
// Good — structured
info!(agent_id = %id, status = %agent.status, "Agent state changed");

// Avoid — unstructured
info!("Agent {} changed to {}", id, agent.status);
```

## Dependencies

All crates use workspace-level dependency versions:

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

Service crates also include:
```toml
tower-http = { version = "0.6", features = ["trace"] }
```
