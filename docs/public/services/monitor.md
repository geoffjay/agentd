# agentd-monitor

System health monitoring and alerting daemon. Watches CPU, memory, disk, and load average metrics at configurable intervals and exposes a REST API for querying current state, triggering on-demand collection, and exporting Prometheus metrics.

## Base URL

```
http://127.0.0.1:17003
```

Port defaults to `17003` in development and `7003` in production, configurable via the `AGENTD_PORT` environment variable.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENTD_PORT` | `17003` | HTTP listen port |
| `AGENTD_COLLECTION_INTERVAL_SECS` | `30` | Seconds between automatic metric collections |
| `AGENTD_CPU_ALERT_THRESHOLD` | `90.0` | CPU usage percentage to trigger an alert |
| `AGENTD_MEMORY_ALERT_THRESHOLD` | `90.0` | Memory usage percentage to trigger an alert |
| `AGENTD_DISK_ALERT_THRESHOLD` | `90.0` | Disk usage percentage to trigger an alert |
| `AGENTD_HISTORY_SIZE` | `120` | Number of metric snapshots to retain in memory |
| `AGENTD_LOG_FORMAT` | `text` | Log format: `text` or `json` |

## Endpoints

### `GET /health`

Standard health check. Returns service name, version, and metrics collection count.

```bash
curl http://localhost:17003/health
```

### `GET /metrics`

Returns the latest system metrics snapshot as JSON. Returns `503` if no collection has run yet.

```bash
curl http://localhost:17003/metrics
```

### `POST /collect`

Triggers an immediate metrics collection and returns the snapshot along with any threshold alerts.

```bash
curl -X POST http://localhost:17003/collect
```

**Response:**

```json
{
  "metrics": { "...": "..." },
  "alerts": [
    {
      "metric": "cpu",
      "current_value": 95.2,
      "threshold": 90.0,
      "message": "CPU usage 95.2% exceeds threshold 90.0%",
      "raised_at": "2026-03-31T12:00:00Z"
    }
  ]
}
```

### `GET /history`

Returns all retained metrics snapshots (up to `AGENTD_HISTORY_SIZE`) as a JSON array, newest first.

```bash
curl http://localhost:17003/history
```

### `GET /status`

Health assessment based on configured thresholds. Returns overall status (`healthy`, `degraded`, or `critical`) and any active alerts.

```bash
curl http://localhost:17003/status
```

### `GET /prom-metrics`

Prometheus exposition format metrics for scraping.

```bash
curl http://localhost:17003/prom-metrics
```

## Data Models

### SystemMetrics

| Field | Type | Description |
|-------|------|-------------|
| `collected_at` | `string` (datetime) | Collection timestamp |
| `cpu` | `CpuMetrics` | CPU utilization metrics |
| `memory` | `MemoryMetrics` | Memory usage metrics |
| `disks` | `DiskMetrics[]` | Per-disk usage metrics |
| `load_average` | `LoadAverage` | System load averages |

### CpuMetrics

| Field | Type | Description |
|-------|------|-------------|
| `usage_percent` | `float` | Global CPU usage 0.0-100.0 |
| `core_count` | `integer` | Number of logical CPU cores |
| `per_core` | `float[]` | Per-core usage percentages |

### MemoryMetrics

| Field | Type | Description |
|-------|------|-------------|
| `total_bytes` | `integer` | Total physical memory in bytes |
| `used_bytes` | `integer` | Used memory in bytes |
| `available_bytes` | `integer` | Available memory in bytes |
| `usage_percent` | `float` | Memory usage percentage 0.0-100.0 |

### DiskMetrics

| Field | Type | Description |
|-------|------|-------------|
| `name` | `string` | Disk device name or label |
| `mount_point` | `string` | Mount point path |
| `total_bytes` | `integer` | Total disk space in bytes |
| `available_bytes` | `integer` | Free space in bytes |
| `used_bytes` | `integer` | Used space in bytes |
| `usage_percent` | `float` | Usage percentage 0.0-100.0 |

### LoadAverage

| Field | Type | Description |
|-------|------|-------------|
| `one` | `float` | 1-minute load average |
| `five` | `float` | 5-minute load average |
| `fifteen` | `float` | 15-minute load average |

### Alert

| Field | Type | Description |
|-------|------|-------------|
| `metric` | `string` | Metric that triggered the alert (e.g., `cpu`, `memory`, `disk:/`) |
| `current_value` | `float` | Current metric value |
| `threshold` | `float` | Threshold that was exceeded |
| `message` | `string` | Human-readable description |
| `raised_at` | `string` (datetime) | Alert timestamp |

## CLI Usage

```bash
# Check service health
agent monitor health

# Get current metrics
agent monitor metrics

# Trigger on-demand collection
agent monitor collect

# View metric history
agent monitor history

# Get overall system status
agent monitor status
```
