# agentd-ui

Static file server and API reverse proxy for the Agent UI dashboard. Serves the built React SPA and proxies API requests to backend services (ask, notify, orchestrator). Implements SPA fallback routing and CORS support.

## Base URL

```
http://127.0.0.1:17009
```

Port defaults to `17009` in development and `7009` in production, configurable via the `AGENTD_PORT` environment variable.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17009` | HTTP listen port |
| `AGENTD_UI_DIR` | `./ui/dist` | Path to built UI assets |
| `AGENTD_ASK_SERVICE_URL` | `http://localhost:7001` | Ask service URL |
| `AGENTD_NOTIFY_SERVICE_URL` | `http://localhost:7004` | Notify service URL |
| `AGENTD_ORCHESTRATOR_SERVICE_URL` | `http://localhost:7006` | Orchestrator service URL |
| `AGENTD_LOG_FORMAT` | `text` | Log format: `text` or `json` |

## Endpoints

### `GET /health`

Simple health check returning `"ok"`.

```bash
curl http://localhost:17009/health
```

### `ANY /api/ask/{path}`

Proxies all requests to the ask service. The `/api/ask/` prefix is stripped before forwarding.

```bash
# Equivalent to GET http://localhost:7001/questions
curl http://localhost:17009/api/ask/questions
```

### `ANY /api/notify/{path}`

Proxies all requests to the notify service. The `/api/notify/` prefix is stripped before forwarding.

```bash
# Equivalent to GET http://localhost:7004/notifications
curl http://localhost:17009/api/notify/notifications
```

### `ANY /api/orchestrator/{path}`

Proxies all requests to the orchestrator service. The `/api/orchestrator/` prefix is stripped before forwarding.

```bash
# Equivalent to GET http://localhost:7006/agents
curl http://localhost:17009/api/orchestrator/agents
```

### `GET /**`

Serves static files from the UI directory. Unmatched paths fall back to `index.html` for SPA client-side routing.

## Proxy Behavior

- Strips the `/api/{service}/` prefix from request paths before forwarding
- Preserves query strings when forwarding
- Forwards all headers except the `host` header
- Supports all HTTP methods (GET, POST, PUT, DELETE, PATCH, etc.)
- Request body size limit: 10 MB
- Returns `502 Bad Gateway` on upstream connection failure
- Returns `400 Bad Request` on request body read failure

## Building the UI

The UI is a React SPA located in the `ui/` directory at the repository root:

```bash
cd ui
bun install
bun run build
```

The built assets are placed in `ui/dist/` by default, which matches the `AGENTD_UI_DIR` default.

## Development

For local development, run the UI dev server separately and point it at the API proxy:

```bash
# Terminal 1: start the UI service (serves built assets and proxies API)
cargo run -p agentd-ui

# Terminal 2: start the React dev server with hot reload
cd ui && bun run dev
```

The React dev server can be configured to proxy API requests to the UI service or directly to individual backend services.
