//! Monitor Service - System health monitoring and alerting daemon.
//!
//! The `agentd-monitor` service provides a REST API for collecting and querying
//! system health metrics including CPU, memory, disk, and load average. It supports
//! configurable thresholds for anomaly detection and optional AI-powered log analysis
//! via the BAML integration.
//!
//! # Features
//!
//! - **System Metrics**: CPU usage, memory usage, disk usage, load averages
//! - **Metrics History**: Configurable in-memory ring buffer of metric snapshots
//! - **Prometheus Export**: `/metrics` endpoint in Prometheus text format
//! - **REST API**: JSON endpoints for health checks and metrics collection
//! - **Configurable Thresholds**: Alert when CPU/memory/disk exceed thresholds
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint that returns service status.
//!
//! ```bash
//! curl http://localhost:17003/health
//! ```
//!
//! ## GET /metrics
//!
//! Returns the latest system metrics snapshot as JSON.
//!
//! ```bash
//! curl http://localhost:17003/metrics
//! ```
//!
//! ## POST /collect
//!
//! Triggers an immediate metrics collection and returns the snapshot.
//!
//! ```bash
//! curl -X POST http://localhost:17003/collect
//! ```
//!
//! ## GET /history
//!
//! Returns all collected metrics snapshots (up to the configured history size).
//!
//! ```bash
//! curl http://localhost:17003/history
//! ```
//!
//! ## GET /status
//!
//! Returns a health assessment based on configured thresholds.
//!
//! ```bash
//! curl http://localhost:17003/status
//! ```
//!
//! # Environment Variables
//!
//! - `PORT` - Port to listen on (default: 17003 dev, 7003 production)
//! - `COLLECTION_INTERVAL_SECS` - Seconds between auto-collections (default: 30)
//! - `CPU_ALERT_THRESHOLD` - CPU usage % to trigger alert (default: 90.0)
//! - `MEMORY_ALERT_THRESHOLD` - Memory usage % to trigger alert (default: 90.0)
//! - `DISK_ALERT_THRESHOLD` - Disk usage % to trigger alert (default: 90.0)
//! - `HISTORY_SIZE` - Number of metric snapshots to retain (default: 120)
//! - `RUST_LOG` - Logging level (default: info)

pub mod api;
pub mod client;
pub mod config;
pub mod error;
pub mod metrics_collector;
pub mod state;
pub mod types;
