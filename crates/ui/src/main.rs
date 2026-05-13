//! agentd-ui — Static file server and API reverse proxy for the Agent UI.
//!
//! Serves the built React SPA and proxies API requests to backend services.
//!
//! **Default port:** 17009 (dev) / 7009 (production)
//!
//! # Usage
//!
//! ```bash
//! # Start with defaults
//! agentd-ui
//!
//! # Override port and UI directory
//! AGENTD_PORT=7009 AGENTD_UI_DIR=/path/to/dist agentd-ui
//! ```

use agentd_common::config::ValidateConfig;
use anyhow::Result;
use ui::config::UiConfig;

#[tokio::main]
async fn main() -> Result<()> {
    agentd_common::server::init_tracing();
    let cfg = UiConfig::load();
    cfg.validate()?;
    ui::run(cfg).await
}
