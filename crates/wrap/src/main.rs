//! agentd-wrap service entry point.
//!
//! Provides a REST API for launching and managing agent sessions. The active
//! execution backend is selected via the `AGENTD_BACKEND` environment variable:
//!
//! | Value        | Backend                      |
//! |--------------|------------------------------|
//! | `tmux`       | tmux sessions (default)      |
//! | `pty`        | In-process PTY               |
//! | `subprocess` | Direct subprocess (SDK-only) |
//!
//! # Running the Service
//!
//! ```bash
//! # PTY backend on default port
//! AGENTD_BACKEND=pty cargo run -p agentd-wrap
//!
//! # tmux backend on a custom port
//! AGENTD_PORT=8080 cargo run -p agentd-wrap
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG`                     — Logging level (default: `info`)
//! - `AGENTD_PORT`                  — Listen port (default: `17005`)
//! - `AGENTD_BACKEND`               — Execution backend: `tmux` | `pty` | `subprocess` (default: `tmux`)
//! - `AGENTD_WRAP_HISTORY_BYTES`    — PTY output ring-buffer size in bytes (default: `524288` / 512 KiB); agentd-wrap PTY backend only
//! - `AGENTD_WRAP_CHANNEL_CAPACITY` — PTY broadcast channel capacity in chunks (default: `256`); must be ≥ 1; agentd-wrap PTY backend only

// Import from the library target — avoids re-declaring modules in the binary and
// triggering dead-code warnings on items that are only used by the library.
use agentd_common::config::ValidateConfig;
use wrap::{
    api::{create_router, AppState},
    backend::{ExecutionBackend, TmuxBackend},
    config::WrapConfig,
    pty::PtyBackend,
    subprocess::SubprocessBackend,
    types::BackendType,
};

use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::Arc;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "wrap")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();
    info!("Starting agentd-wrap service...");

    let cfg = WrapConfig::load();
    cfg.validate()?;

    // --- Select execution backend ---
    // Unrecognised AGENTD_BACKEND values cause an immediate startup failure.
    let backend_type = BackendType::from_env_strict()?;
    info!("Using execution backend: {}", backend_type);

    let exec_backend: Arc<dyn ExecutionBackend> = match &backend_type {
        BackendType::Pty => {
            info!("Initialising PTY backend");
            info!(
                "PTY ring-buffer: history_bytes={}, channel_capacity={}",
                cfg.history_bytes, cfg.channel_capacity
            );
            Arc::new(PtyBackend::new_with_config("agentd", cfg.channel_capacity, cfg.history_bytes))
        }
        BackendType::Tmux => {
            info!("Initialising tmux backend");
            Arc::new(TmuxBackend::new("agentd"))
        }
        BackendType::Subprocess => {
            info!("Initialising subprocess backend");
            Arc::new(SubprocessBackend::new("agentd"))
        }
    };

    let state = AppState { backend: exec_backend, backend_type };

    // --- Build router ---
    let metrics_handle = init_metrics();
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(state)
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::trace_layer());

    // --- Bind and serve ---
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Wrap API server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
