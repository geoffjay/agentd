# agentd Observability Guide

The agentd local observability stack provides real-time visibility into all
running services using Prometheus for metrics collection and Grafana for
visualization.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  agentd services                │
│                                                 │
│  orchestrator :17006   communicate :17010       │
│  notify       :17004   ask         :17001       │
│  wrap         :17005   memory      :17008       │
│                   │ /metrics                    │
└───────────────────┼─────────────────────────────┘
                    │ scrape (15s)
                    ▼
           ┌─────────────────┐
           │   Prometheus    │  :9090
           │  (time-series   │
           │   database)     │
           └────────┬────────┘
                    │ PromQL
                    ▼
           ┌─────────────────┐
           │     Grafana     │  :3000
           │  (dashboards +  │
           │  provisioning)  │
           └─────────────────┘
```

All agentd services expose a Prometheus-compatible `/metrics` endpoint.
Prometheus scrapes these every 15 seconds and stores the time-series data
locally. Grafana connects to Prometheus as a data source and renders
pre-built dashboards that are auto-provisioned from this repository.

## Quick Start

```bash
# Install and configure everything in one command
./infra/setup.sh

# Open the dashboards
open http://localhost:3000
```

The setup script:
1. Installs Prometheus and Grafana via Homebrew (if not already installed)
2. Configures Prometheus to scrape all agentd service `/metrics` endpoints
3. Configures Grafana provisioning to auto-load data sources and dashboards
4. Registers both services as macOS launchd agents (start at login, stay alive)
5. Waits for services to start and prints status

Default credentials: `admin` / `admin` — change after first login.

## Manual Setup

If you prefer to configure services manually or need to understand the
individual components:

- **[Prometheus setup](prometheus-setup.md)** — installation, configuration, launchd plist
- **[Grafana setup](grafana-setup.md)** — installation, provisioning, launchd plist, troubleshooting

## Pre-Built Dashboards

All dashboards are provisioned automatically and appear in the **agentd**
folder in Grafana.

### Service Overview

**UID:** `agentd-service-overview` | **Tags:** `agentd`, `overview`

Shows the health and activity of all agentd services at a glance:

- Per-service up/down status (green/red stat panel per service)
- HTTP request rate over time by service

Use this dashboard to confirm all services are running and to spot
unusual request rate spikes.

### Agent Activity

**UID:** `agentd-agent-activity` | **Tags:** `agentd`, `agents`

Focused on agent and messaging metrics:

- Active WebSocket connections (communicate service)
- Active agent count and total agents created (orchestrator)
- Total usage session cost in USD
- Messages delivered, dropped, and queued over time
- Workflow dispatches by status (completed, failed, pending)
- Usage session cost accumulation over time

Use this dashboard to monitor agent health, detect connection loss,
and track token spending.

### Workflow Execution

**UID:** `agentd-workflow-execution` | **Tags:** `agentd`, `workflows`

Focused on workflow and notification pipeline:

- Total dispatches, failed dispatches, pending notifications, pending approvals
- Dispatch rate by status (timeseries)
- Dispatch status breakdown (pie chart)
- Notifications by priority over time

Use this dashboard to diagnose workflow failures and notification backlogs.

### Experiment Tracking

**UID:** `agentd-experiment-tracking` | **Tags:** `agentd`, `experiments`

Filterable by the `experiment` Prometheus label:

- Variable dropdown populates dynamically from active experiment labels
- Dispatch count and failure count for selected experiment(s)
- Cost accumulated during the experiment
- Median dispatch duration
- Cost accumulation over time (one series per experiment)
- Side-by-side comparison table across all experiments

> **Note:** This dashboard requires experiment label injection (issue #833).
> Until that is implemented, panels will show "No data".

## Using Projects and Experiments with Observability

When an experiment is active, Prometheus metrics are tagged with an
`experiment` label (e.g., `experiment="exp-q1-cost-reduction"`). This enables:

- **Filtering**: Use the Experiment dropdown in the Experiment Tracking dashboard
  to see metrics for a specific experiment only.
- **Comparing**: Select multiple experiments in the dropdown to overlay their
  metrics side-by-side.
- **Cost tracking**: The `experiment_cost_usd_total` counter accumulates spend
  attributed to each experiment.

## Removing the Stack

```bash
# Stop services and remove plists (preserves data)
./infra/teardown.sh

# Also delete all logs, Prometheus history, and Grafana state
./infra/teardown.sh --purge
```

## Configuration Files

| File | Purpose |
|------|---------|
| `infra/prometheus/prometheus.yml` | Prometheus scrape config (service targets) |
| `infra/grafana/grafana.ini` | Grafana settings (ports, provisioning path, data dir) |
| `infra/grafana/provisioning/datasources/agentd-prometheus.yml` | Auto-configure Prometheus datasource |
| `infra/grafana/provisioning/dashboards/agentd.yml` | Auto-load dashboards from `infra/grafana/dashboards/` |
| `infra/grafana/dashboards/*.json` | Pre-built dashboard definitions |
| `infra/launchd/com.agentd.prometheus.plist` | macOS launchd plist for Prometheus |
| `infra/launchd/com.agentd.grafana.plist` | macOS launchd plist for Grafana |

## Extending the Stack

### Adding a custom dashboard

1. Build your dashboard in Grafana UI (connected to `agentd-prometheus` datasource)
2. Export as JSON: **Dashboard settings → JSON Model → Copy to clipboard**
3. Save to `infra/grafana/dashboards/my-dashboard.json`
4. Grafana reloads provisioned dashboards every 30s automatically

### Adding alerting rules

1. Create `infra/prometheus/alert_rules.yml` with your rules
2. Uncomment the `rule_files:` section in `infra/prometheus/prometheus.yml`
3. Reload Prometheus: `curl -X POST http://localhost:9090/-/reload`

### Changing scrape intervals

Edit `infra/prometheus/prometheus.yml`:
- `global.scrape_interval` — default for all jobs
- Per-job override: add `scrape_interval: 5s` inside a `scrape_configs` entry

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| Grafana shows "No data" | Prometheus not running | `curl http://localhost:9090/-/healthy` |
| Prometheus target is DOWN | agentd service not running | `cargo run -p agentd-orchestrator` |
| Grafana dashboards not loading | Wrong provisioning path | Check `provisioning` in `grafana.ini` |
| Port 9090 / 3000 in use | Another process | `lsof -i :9090` or `lsof -i :3000` |
| Service not starting at login | Plist not loaded | `launchctl list \| grep agentd` |

For detailed troubleshooting, see the individual setup guides:

- [prometheus-setup.md](prometheus-setup.md#troubleshooting)
- [grafana-setup.md](grafana-setup.md#troubleshooting)
