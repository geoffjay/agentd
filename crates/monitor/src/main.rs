//! agentd-monitor — System monitoring and alerting service.
//!
//! Watches system metrics (CPU, memory, disk, load average) and exposes a
//! REST API for querying current state and triggering on-demand collection.
//!
//! **Default port:** 17003 (dev) / 7003 (production)
//!
//! # Usage
//!
//! ```bash
//! # Start with defaults (port 17003, 30-second collection interval)
//! agentd-monitor
//!
//! # Override port and interval via environment variables
//! PORT=7003 COLLECTION_INTERVAL_SECS=60 agentd-monitor
//!
//! # JSON structured logging
//! LOG_FORMAT=json agentd-monitor
//! ```

use anyhow::Result;
use monitor::config::MonitorConfig;

#[tokio::main]
async fn main() -> Result<()> {
    agentd_common::server::init_tracing();
    monitor::run(MonitorConfig::from_env()).await
}
