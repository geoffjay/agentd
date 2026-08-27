# Prometheus Setup for agentd

This guide walks through installing, configuring, and starting Prometheus to
scrape metrics from all running agentd services.

## Prerequisites

- macOS with Homebrew installed
- One or more agentd services running on their default dev ports (170xx)

## Installation

```bash
brew install prometheus
```

Verify the installation:

```bash
prometheus --version
```

## Configuration

The `infra/prometheus/prometheus.yml` file in this repository configures
Prometheus to scrape the following agentd services:

| Service       | Port  | Endpoint              |
|---------------|-------|-----------------------|
| orchestrator  | 17006 | `/metrics`            |
| communicate   | 17010 | `/metrics`            |
| notify        | 17004 | `/metrics`            |
| ask           | 17001 | `/metrics`            |
| wrap          | 17005 | `/metrics`            |
| memory        | 17008 | `/metrics`            |

> **Note:** The monitor service (port 17003) exposes system metrics via a
> REST JSON endpoint (`/metrics`) rather than Prometheus text format. Its
> metrics are not scraped directly by Prometheus. The `/metrics` path on
> all other services returns standard Prometheus text format.

## Quick Start (Manual)

Run Prometheus directly, pointing at the config file:

```bash
prometheus \
  --config.file=infra/prometheus/prometheus.yml \
  --storage.tsdb.path=/opt/homebrew/var/prometheus \
  --web.listen-address=127.0.0.1:9090
```

Open the Prometheus UI at <http://localhost:9090> and navigate to
**Status → Targets** to verify all scrape targets are `UP`.

## Running as a macOS launchd Service

The launchd plist at `infra/launchd/com.agentd.prometheus.plist` runs
Prometheus automatically at login and keeps it alive.

### One-time setup

> **Tip:** The `infra/setup.sh` script automates these steps. Run it
> instead of following the manual steps below.

1. **Create required directories:**

   ```bash
   mkdir -p ~/Library/Logs/agentd
   sudo mkdir -p /usr/local/var/prometheus   # or /opt/homebrew/var/prometheus
   ```

2. **Edit the plist to set the correct repo path.**

   Open `infra/launchd/com.agentd.prometheus.plist` and update the
   `--config.file` argument to the absolute path of your repo's
   `infra/prometheus/prometheus.yml`.

   Example (replace `<YOUR_USER>` and `<REPO_PATH>`):
   ```xml
   <string>--config.file=/Users/<YOUR_USER>/<REPO_PATH>/infra/prometheus/prometheus.yml</string>
   ```

3. **Copy the plist to LaunchAgents:**

   ```bash
   cp infra/launchd/com.agentd.prometheus.plist ~/Library/LaunchAgents/
   ```

4. **Load the service:**

   ```bash
   launchctl load ~/Library/LaunchAgents/com.agentd.prometheus.plist
   ```

5. **Verify it started:**

   ```bash
   launchctl list | grep prometheus
   curl -s http://localhost:9090/-/ready
   ```

### Stopping the service

```bash
launchctl unload ~/Library/LaunchAgents/com.agentd.prometheus.plist
```

### Viewing logs

```bash
tail -f /Users/Shared/agentd/logs/prometheus.log
```

## Reloading Configuration

Prometheus supports hot-reload without restart when `--web.enable-lifecycle`
is set (it is set in the plist):

```bash
curl -X POST http://localhost:9090/-/reload
```

## Port Customization

If you run services on non-default ports (e.g., for parallel dev environments),
edit `infra/prometheus/prometheus.yml` and update the `targets` arrays.

For production environments using the 70xx port range, replace all `170xx`
ports with `70xx`:

```yaml
- targets: ['127.0.0.1:7006']  # production orchestrator
```

## Troubleshooting

### Target shows "connection refused"

The agentd service is not running on that port. Start it with:

```bash
cargo run -p agentd-orchestrator  # or whichever service is down
```

### Target shows "context deadline exceeded"

The service is running but `/metrics` is slow or blocked. Check service logs.

### Prometheus binary not found

Confirm the brew path:

```bash
which prometheus
# /opt/homebrew/bin/prometheus (Apple Silicon)
# /usr/local/bin/prometheus    (Intel)
```

Update the `ProgramArguments` path in the plist accordingly.

### Port 9090 already in use

Another Prometheus instance may be running. Find and stop it:

```bash
lsof -i :9090
```
