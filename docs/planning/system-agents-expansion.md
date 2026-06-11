# System Agents Expansion

Tracking document for the `feature/system-agents` stack: a definitions
registry for built-in agents, MCP server support for agent sessions, and
three new built-ins (diagnostician, architect, analyst) backed by a
Prometheus query layer in the monitor service.

## Motivation

agentd shipped with a single hardcoded built-in (`agentd-system`). Adding
more built-ins surfaced structural gaps:

- Bootstrap restarted existing built-ins with their **stored** config, so
  prompt/policy changes in a release never reached deployments.
- The system prompt's service table had drifted from the real port defaults
  (notify 17005 vs actual 17004, wrap 17007 vs actual 17005; ask, hook, and
  monitor missing).
- `AgentConfig` had no MCP server support, and tool policies could not
  allowlist MCP tool families (plain-name patterns matched exactly).
- The monitor service collected host metrics nobody consumed, while the MCP
  server's Prometheus tooling scraped raw `/metrics` endpoints with no
  history and no PromQL.

## Stack

PRs merge into `feature/system-agents`, never directly to `main`.

1. **fix(mcp): scrape monitor Prometheus metrics from /prom-metrics** — the
   monitor serves Prometheus text at `/prom-metrics`; `/metrics` is JSON.
2. **System-agent registry** — `SystemAgentDef` + `builtin_agent_defs()`;
   config-drift refresh (normalizing runtime-mutated fields such as
   `interactive` on PTY backends and the cwd-derived `working_dir`); lazy
   built-ins (dormant record, spawn on first message); orphan cleanup.
3. **Generated service table** — the agentd-system prompt's service
   inventory and example ports render from the loaded `ServicesConfig`.
4. **Tool-policy name globs** — trailing-`*` matching on plain tool names so
   policies can allowlist `mcp__agentd__*` families. Bare `*` stays inert.
5. **`mcp_servers` agent config** — per-agent MCP config files written at
   spawn/restart, launched with `--mcp-config <file> --strict-mcp-config`;
   full surface propagation (API, CLI, templates, schema, UI) plus env
   redaction. Docker backends are warn-and-skip for now.
6. **agentd-diagnostician** — lazy built-in, MCP-only read policy by verb
   prefix; remediation tools gated behind
   `AGENTD_DIAGNOSTICIAN_REMEDIATION=1`.
7. **MCP creation tools** — `create_agent` (no env — secrets must not
   transit MCP), `create_workflow`/`update_workflow`/`set_workflow_enabled`/
   `trigger_workflow`/`delete_workflow`; fixes the internally-tagged
   ToolPolicy wire shape and two POST-vs-PUT method bugs in existing
   lifecycle tools.
8. **agentd-architect** — lazy built-in that provisions agents/workflows via
   the API only; repository `.agentd/*.yml` templates remain human-gated.
9. **Monitor query layer** — hand-rolled Prometheus HTTP API client, a
   14-entry curated PromQL catalog with `$__window` validation, and
   `GET /queries` / `GET /queries/{name}` / `GET /query` endpoints
   (404/400/502 error contract).
10. **`query_metrics` MCP tool** — catalog rendering and instant/range
    result summaries via the monitor.
11. **agentd-analyst** — lazy, read-only built-in with catalog baselines and
    investigation playbooks; escalates via notifications and the `system`
    room.

## Follow-ups (out of stack scope)

- Docker delivery of MCP config files (mount-based; inline JSON would leak
  env values into the UI-visible launch command).
- A cron workflow dispatching a daily routine review to the analyst —
  ideally created by the architect as dogfood.
- Migrating monitor's threshold/prometheus_url settings into the shared
  config schema (tracked as TODO(#1201) comments).
