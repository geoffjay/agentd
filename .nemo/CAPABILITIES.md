# agentd nemo application - capabilities and gaps

## What works today

The nemo configuration in `.nemo/` provides a dashboard for the agentd
orchestrator using nemo's built-in capabilities plus the new HTTP functions:

| Feature | How | Status |
|---------|-----|--------|
| Health monitoring | HTTP polling `GET /health` | works |
| Agent list | HTTP polling `GET /agents` | works |
| Workflow list | HTTP polling `GET /workflows` | works |
| Auto-refresh | Timer-based polling (configurable interval) | works |
| Navigation | RHAI `set_component_property` for page visibility | works |
| Data transforms | RHAI functions for formatting API responses | works |
| Status cards | Template-based dashboard with live data binding | works |
| Create agent | `http_post()` from RHAI handler | works |
| Refresh agents | `http_get()` from RHAI handler | works |
| Refresh workflows | `http_get()` from RHAI handler | works |
| Terminate agent | `http_delete()` from RHAI handler | implementable |
| Update workflow | `http_put()` from RHAI handler | implementable |
| Delete workflow | `http_delete()` from RHAI handler | implementable |

## HTTP functions added to nemo

The following RHAI functions were added to `nemo-extension`:

```rhai
// All return a map: #{status: 200, body: ..., ok: true} or #{error: "..."}
let result = http_get("http://127.0.0.1:17006/agents");
let result = http_post("http://127.0.0.1:17006/agents", body_json);
let result = http_put("http://127.0.0.1:17006/workflows/uuid", body_json);
let result = http_delete("http://127.0.0.1:17006/agents/uuid");

if result.ok {
    let data = result.body;  // parsed JSON
}
```

## Remaining gap: dynamic WebSocket source registration

### The problem

When an agent is created, we'd want to subscribe to its WebSocket at
`ws://127.0.0.1:17006/ws/{agent_id}` to stream live output into the activity
feed. Currently, nemo data sources are static (defined in XML at load time) and
cannot be registered at runtime.

### Recommended solution: multiplexed stream endpoint

Add a single WebSocket endpoint to the orchestrator that multiplexes all agent
output:

```
ws://127.0.0.1:17006/ws/stream
```

This endpoint would:
1. Forward all connected agents' NDJSON messages, tagged with agent_id
2. Allow the nemo app to use a single static WebSocket data source
3. Emit messages like: `{"agent_id": "uuid", "type": "assistant", "content": "..."}`

This avoids the dynamic registration problem entirely. The nemo app would
configure one WebSocket source in `app.xml`:

```xml
<source name="agent_stream" type="websocket"
        url="${var.ws_base}/ws/stream" />
```

And use a RHAI transform to filter/format messages per agent.

### Alternative: runtime source registration in nemo

Add a RHAI function `register_source(name, type, config)` to `nemo-data` that
creates and starts a new data source at runtime. This is more general but
requires deeper changes to the data flow engine.

## Running the application

```sh
# From the agentd repo root (orchestrator must be running):
nemo --app-config .nemo/app.xml
```

## File structure

```
.nemo/
  app.xml               main application config
  CAPABILITIES.md       this file
  scripts/
    transforms.rhai     data transform functions (format API responses)
    handlers.rhai       UI event handlers (navigation, HTTP actions)
  templates/
    cards.xml           reusable UI templates (nav_item, status_card, etc.)
    data.xml            data source notes and placeholders
```
