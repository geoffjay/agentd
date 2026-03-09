//! agentd-hook — Shell and git hook integration daemon.
//!
//! Receives hook events from shell and git integrations and creates
//! notifications when user intervention may be required.
//!
//! **Default port:** 17002 (dev) / 7002 (production)
//!
//! # Usage
//!
//! ```bash
//! # Start with defaults (port 17002)
//! agentd-hook
//!
//! # Override port and thresholds via environment variables
//! PORT=7002 HOOK_LONG_RUNNING_THRESHOLD_MS=60000 agentd-hook
//!
//! # JSON structured logging
//! LOG_FORMAT=json agentd-hook
//! ```

use anyhow::Result;
use hook::config::HookConfig;

#[tokio::main]
async fn main() -> Result<()> {
    agentd_common::server::init_tracing();
    hook::run(HookConfig::from_env()).await
}
