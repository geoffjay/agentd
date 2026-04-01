# Grafana Setup for agentd

This guide covers installing Grafana, connecting it to the local Prometheus
data source, and loading the pre-built agentd dashboards.

## Prerequisites

- Prometheus running and scraping agentd services (see [prometheus-setup.md](prometheus-setup.md))
- macOS with Homebrew installed

## Installation

```bash
brew install grafana
```

Verify the installation:

```bash
grafana --version
```

## Quick Start (Manual)

1. **Point Grafana at the agentd provisioning config:**

   ```bash
   grafana server \
     --homepath /opt/homebrew/share/grafana \
     --config $(pwd)/infra/grafana/grafana.ini
   ```

   > Replace `$(pwd)` with the absolute path to your repo root if running
   > from a different directory.

2. **Open Grafana** at <http://localhost:3000>

   Default credentials: `admin` / `admin` (change on first login)

3. **Verify provisioning:**
   - Navigate to **Connections → Data sources** - `agentd-prometheus` should appear
   - Navigate to **Dashboards → Browse → agentd** - all four dashboards should load

## Pre-Built Dashboards

| Dashboard | UID | Description |
|-----------|-----|-------------|
| Service Overview | `agentd-service-overview` | Up/down status, HTTP request rates per service |
| Agent Activity | `agentd-agent-activity` | WebSocket connections, dispatches, messaging, costs |
| Workflow Execution | `agentd-workflow-execution` | Dispatch status, notification activity, approvals |
| Experiment Tracking | `agentd-experiment-tracking` | Filter by experiment, cost accumulation, comparison |

### Experiment Tracking dashboard

The **Experiment Tracking** dashboard includes a variable dropdown that
dynamically populates from the `experiment` label on Prometheus metrics.
When experiment labels are emitted (see issue #833), you can:

- Select one or more experiments to compare
- See dispatch rates and cost accumulation side-by-side
- Use the **All** option to aggregate across all experiments

> **Note:** The `experiment_dispatches_total` and `experiment_cost_usd_total`
> metrics require issue #833 (experiment label injection) to be implemented.
> Until then, the experiment dashboard panels will show "No data".

## Running as a macOS launchd Service

The launchd plist at `infra/launchd/com.agentd.grafana.plist` runs Grafana
automatically at login.

### One-time setup

> **Tip:** The `infra/setup.sh` script automates these steps.

1. **Create required directories:**

   ```bash
   mkdir -p /Users/Shared/agentd/logs /Users/Shared/agentd/grafana-data
   ```

2. **Edit path placeholders in config files.**

   The `infra/grafana/grafana.ini` and `infra/launchd/com.agentd.grafana.plist`
   reference `/Users/Shared/agentd/` as the base path. Update these to your
   actual repo root:

   ```bash
   REPO_ROOT=$(pwd)
   sed -i '' "s|/Users/Shared/agentd|$REPO_ROOT|g" \
     infra/grafana/grafana.ini \
     infra/launchd/com.agentd.grafana.plist
   ```

3. **Copy the plist to LaunchAgents:**

   ```bash
   cp infra/launchd/com.agentd.grafana.plist ~/Library/LaunchAgents/
   ```

4. **Load the service:**

   ```bash
   launchctl load ~/Library/LaunchAgents/com.agentd.grafana.plist
   ```

5. **Verify it started:**

   ```bash
   launchctl list | grep grafana
   curl -s http://localhost:3000/api/health | python3 -m json.tool
   ```

### Stopping the service

```bash
launchctl unload ~/Library/LaunchAgents/com.agentd.grafana.plist
```

### Viewing logs

```bash
tail -f /Users/Shared/agentd/logs/grafana.log
```

## Provisioning Details

### Data source (`infra/grafana/provisioning/datasources/agentd-prometheus.yml`)

Automatically configures Grafana to connect to `http://localhost:9090` as
the default Prometheus data source. The UID `agentd-prometheus` is used in
all dashboard JSON files as a stable reference.

### Dashboard provider (`infra/grafana/provisioning/dashboards/agentd.yml`)

Watches the `infra/grafana/dashboards/` directory and reloads dashboards
every 30 seconds when JSON files change. This means you can edit dashboard
JSON and see changes without restarting Grafana.

## Troubleshooting

### "No data" on all panels

Prometheus is not running or no agentd services are up. Check:

```bash
curl http://localhost:9090/-/ready
curl http://localhost:17006/metrics  # orchestrator
```

### Data source connection failed

Grafana cannot reach Prometheus at `http://localhost:9090`. Verify Prometheus
is running:

```bash
launchctl list | grep prometheus
curl http://localhost:9090/-/healthy
```

### Dashboards not appearing

Check the provisioning path in `grafana.ini` is absolute and correct:

```bash
grep provisioning infra/grafana/grafana.ini
ls infra/grafana/provisioning/dashboards/
```

Grafana logs will show provisioning errors:

```bash
grep -i "provision\|error" /Users/Shared/agentd/logs/grafana.log | tail -20
```

### Port 3000 already in use

```bash
lsof -i :3000
```

Change `http_port` in `infra/grafana/grafana.ini` if needed.

### Grafana binary not found

```bash
which grafana
# /opt/homebrew/bin/grafana (Apple Silicon)
# /usr/local/bin/grafana    (Intel)
```

Update the `ProgramArguments` path in the plist accordingly.
