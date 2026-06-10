# Instrumentation and API Exposure Gaps

This document tracks areas of the service APIs that would benefit from additional
tracking, aggregation, or exposure. It was produced during a review of the service
crate APIs (`crates/*`) against the needs of the dashboard UI (`ui/`). Each item
lists the gap, a proposed change, and the dashboard or operational capability it
would unlock.

Related: the Prometheus/Grafana stack documented in
[`docs/observability/`](observability/README.md) already scrapes the services'
Prometheus metrics. This document focuses on what the *APIs* expose, since the
web dashboard consumes JSON endpoints directly rather than PromQL.

The dashboard currently works around several of these gaps by fetching full list
endpoints and aggregating client-side (counting statuses, bucketing `created_at`
timestamps by hour). That works at today's data volumes but does not scale: it
transfers full rows to compute a single number, and it is capped by pagination
limits (`limit` max 200), so counts silently saturate once a list exceeds one page.

## Cross-cutting

### 1. Aggregate stats endpoints

Only `notify` (`GET /notifications/count`) and orchestrator queues
(`GET /queues/{name}/stats`) expose aggregate counts. Every other "how many X by
status" question requires listing and counting client-side.

**Proposed:** a `GET /stats` (or `/counts`) endpoint per service returning counts
grouped by status:

- orchestrator: agents by status, approvals by status, workflows enabled/disabled,
  dispatches by outcome (last 24h / 7d)
- ask: questions by status (`pending`, `answered`, `dismissed`)
- communicate: rooms, participants, messages totals
- memory: memories by type and visibility

**Unlocks:** accurate overview stat cards with one cheap request per service, no
pagination saturation.

### 2. Time-bucketed activity endpoints

All major entities carry `created_at`/`updated_at`, but no service can return
counts bucketed over time. The dashboard's activity-over-time chart buckets
client-side from list responses, which is both expensive and truncated to the
first page of results.

**Proposed:** `GET /stats/activity?bucket=hour&since=<rfc3339>` per service
returning `[{ bucket: "2026-06-10T14:00:00Z", count: n }]`. Highest value in:

- notify (notifications created)
- ask (questions created / answered)
- orchestrator (dispatches started / completed, agent state transitions)
- communicate (messages sent)
- hook (commands run, failures)

**Unlocks:** correct, cheap time-series charts of system activity at any volume.

### 2b. Unified event / audit feed

Related but distinct from bucketed counts: the dashboard's "recent activity" feed
is synthesized client-side from four list endpoints (notifications, questions,
agents, and notification history). Beyond the request overhead, this only sees the
*latest* state of each row - an agent that transitioned twice within a poll
interval appears once, and past transitions are invisible.

**Proposed:** a `GET /events?since=<rfc3339>&limit=` feed (orchestrator is the
natural home) recording state *transitions* as append-only events: agent status
changes, dispatch start/finish, approval created/resolved, notification created.

**Unlocks:** an accurate activity feed from one request instead of four polls;
no missed transitions.

### 3. Uptime / started_at in health responses

`HealthResponse` has a `details` map but no service reports its start time or
uptime, so the UI cannot show "restarted 2 min ago" or flag crash loops.

**Proposed:** include `started_at` (RFC3339) and `uptime_secs` in every service's
`/health` details.

**Unlocks:** uptime column in the service health grid; restart detection.

### 4. Mirror key Prometheus gauges in JSON

Services register rich metrics with the `metrics` crate (counters like
`agents_created_total`, gauges like `approvals_pending`), and Prometheus scrapes
them (see [`docs/observability/`](observability/README.md)). However, the web
dashboard does not consume Prometheus format, so none of those values are
reachable from the UI. The most useful gauges should also be surfaced through
plain JSON endpoints (see item 1) so both consumers see the same numbers.

## orchestrator

### 5. Dispatch / workflow outcome aggregates

`GET /workflows/{id}/history` returns paginated dispatches per workflow, but there
is no aggregate view: no success/failure rate, no average runtime, no global
"dispatches in the last 24h" across workflows.

**Proposed:**

- `GET /dispatches?since=&status=` - global dispatch list (currently reachable
  only per-workflow)
- `GET /dispatches/stats` - counts by outcome, average/percentile duration,
  bucketed by hour
- metrics: `dispatches_total{outcome}`, `dispatch_duration_seconds` histogram

**Unlocks:** a workflow pipeline health panel (success rate, throughput, slowest
workflows) - currently impossible without N+1 history requests.

### 6. Agent failure tracking

Agent status is a point-in-time value. There is no counter of failures, restarts
per agent, or last-error reason exposed via the API (`agents_restarted_total`
exists as a metric but is not queryable per agent).

**Proposed:** per-agent `failure_count`, `restart_count`, `last_error`,
`last_state_change_at` fields on `AgentResponse` (or a `GET /agents/{id}/stats`),
plus `agents_failed_total{agent}` counter.

**Unlocks:** "which agents are flapping" on the dashboard; alerting on crash loops.

### 7. Approval latency

Approvals expose `created_at` but not resolution time, so approval SLA (how long
requests wait for a human) cannot be charted.

**Proposed:** add `resolved_at` to `PendingApproval`; metric
`approval_resolution_seconds` histogram; include average pending age in
`GET /approvals` responses or a stats endpoint.

**Unlocks:** "approvals waiting > 1h" alerts and a latency trend chart.

### 8. Per-agent usage history and aggregate usage

`GET /agents/{id}/usage` returns lifetime totals (tokens, cost). There is no
history, so cost over time and cost per day cannot be charted. There is also no
cross-agent aggregate: the dashboard's "total cost" stat issues one usage request
per agent on every poll (up to 200 requests every 30 seconds at the pagination
cap).

**Proposed:** persist usage snapshots (or deltas per session) and expose
`GET /agents/{id}/usage/history?bucket=day`; aggregate cross-agent endpoint
`GET /usage/stats`.

**Unlocks:** cost trend chart, per-agent cost ranking, budget alerting.

## monitor

### 9. History durability and query parameters

`GET /history` returns the in-memory ring buffer: it is lost on restart, has a
fixed size (120 snapshots, about one hour at the 30s collection interval), and
supports no `since`/`limit`/downsampling parameters. The dashboard charts
whatever happens to be in the buffer; a true 24h system-metrics chart is not
possible today.

**Proposed:** `since`, `limit`, and `step` (downsample) query params; optionally
persist snapshots to SQLite like the other services so history survives restarts.

**Unlocks:** stable 24h system charts regardless of service restarts; cheap
long-range queries.

### 10. Per-service process metrics

Monitor reports host-level CPU/memory/disk only. There is no per-service (or
per-agent-process) resource usage, so a runaway agent cannot be identified from
the dashboard.

**Proposed:** collect per-process metrics for the agentd services (and optionally
wrapped agent sessions) keyed by service name: `process_cpu_percent{service}`,
`process_memory_bytes{service}`; expose in `/metrics` JSON.

**Unlocks:** per-service resource breakdown chart; identifying which component is
consuming resources.

## notify

### 11. Time-bucketed notification counts

`GET /notifications/count` gives status distribution; combined with item 2's
activity buckets this service is otherwise well covered. Two smaller gaps:

- the count endpoint reports by status only - no priority breakdown, so the
  dashboard's priority chart lists actionable notifications and counts
  client-side
- `/notifications/history` and `/notifications/actionable` accept no time-range
  filter (`created_after`), so the 24h activity chart fetches the first page and
  hopes it covers the window

## ask

### 12. Question stats and answer latency

`GET /questions` supports filters but there is no count endpoint and no exposed
answer latency, even though `created_at` and `answered_at` both exist. The
dashboard's pending-questions stat currently requests the list with `limit=1`
purely to read the `total` field.

**Proposed:** `GET /questions/count` (by status); include
`answer_latency_secs` derived field or a stats endpoint with average/median time
to answer.

**Unlocks:** pending question stat without listing; "how long do agents wait on
humans" trend.

## hook

### 13. Event aggregates

`GET /events` returns recent raw events (shell/git, `exit_code`, `duration_ms`)
with a 500-row cap and no filters.

**Proposed:** `kind`, `since`, and `failed_only` query params; a
`GET /events/stats` with command counts, failure rate, and long-running command
count bucketed by hour.

**Unlocks:** a useful "Hooks" dashboard panel (failure-rate trend, slow commands)
instead of the current placeholder.

## communicate

### 14. Message rate exposure

Message counts exist only as per-room Prometheus metrics. There is no API to ask
"messages in the last hour across rooms" without paging every room's messages.

**Proposed:** `GET /stats` with total rooms/participants/messages and messages
bucketed by hour (item 2).

**Unlocks:** inter-agent communication activity on the dashboard activity chart
without N requests.

## wrap

### 15. Session lifecycle history

`GET /sessions` lists live sessions only; once a session ends it disappears, with
no record of when it started or stopped.

**Proposed:** include `started_at` in `SessionInfo`; keep a short ring buffer of
ended sessions (`GET /sessions/history`); metric `sessions_total{backend}`.

**Unlocks:** session churn visibility; correlating agent failures with session
restarts.

## memory

### 16. Store size and growth

Health reports backend status but not store size. No count endpoint exists.

**Proposed:** include memory count and store size in `/health` details or a
`GET /stats` (count by type/visibility, growth per day).

**Unlocks:** memory growth trend; spotting runaway memory creation by an agent.

## Suggested priority order

| Priority | Item | Rationale |
|----------|------|-----------|
| 1 | Aggregate stats endpoints (1) | Removes the dashboard's biggest workaround; cheap to add per service |
| 2 | Time-bucketed activity (2) | Powers every time-series chart correctly at scale |
| 3 | Dispatch aggregates (5) | Workflow health is currently a blind spot |
| 4 | Monitor history params/persistence (9) | System chart stability |
| 5 | Agent failure tracking (6) | Operational visibility into flapping agents |
| 6 | Approval latency (7) | Human-in-the-loop SLA |
| 7 | Remaining per-service items | Incremental |
