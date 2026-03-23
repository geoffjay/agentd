//! agentd-wrap service entry point.
//!
//! Provides a REST API for launching and managing agent sessions. The active
//! execution backend is selected via the `AGENTD_BACKEND` environment variable:
//!
//! | Value    | Backend                 |
//! |----------|-------------------------|
//! | `tmux`   | tmux sessions (default) |
//! | `docker` | Docker containers       |
//! | `pty`    | In-process PTY          |
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
//! - `RUST_LOG`        — Logging level (default: `info`)
//! - `AGENTD_PORT`     — Listen port (default: `17005`)
//! - `AGENTD_BACKEND`  — Execution backend: `tmux` | `docker` | `pty` (default: `tmux`)

// Import from the library target — avoids re-declaring modules in the binary and
// triggering dead-code warnings on items that are only used by the library.
use wrap::{
    api::{create_router, AppState},
    backend::{ExecutionBackend, TmuxBackend},
    docker::{DockerBackend, DEFAULT_IMAGE},
    pty::PtyBackend,
    types::BackendType,
};

use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::{env, sync::Arc};
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

    // --- Select execution backend ---
    let backend_type = BackendType::from_env();
    info!("Using execution backend: {}", backend_type);

    let exec_backend: Arc<dyn ExecutionBackend> = match &backend_type {
        BackendType::Pty => {
            info!("Initialising PTY backend");
            Arc::new(PtyBackend::new("agentd"))
        }
        BackendType::Docker => {
            info!("Initialising Docker backend");
            // Fail loudly — an explicit AGENTD_BACKEND=docker request must not
            // silently degrade to a tmux backend (no container isolation, no
            // network policies). Fix the Docker configuration and restart.
            let b = DockerBackend::new("agentd", DEFAULT_IMAGE).map_err(|e| {
                anyhow::anyhow!(
                    "AGENTD_BACKEND=docker requested but Docker initialisation failed: {e}\n\
                     Fix the Docker configuration and restart the service."
                )
            })?;
            Arc::new(b)
        }
        BackendType::Tmux => {
            info!("Initialising tmux backend");
            Arc::new(TmuxBackend::new("agentd"))
        }
    };

    let state = AppState { backend: exec_backend, backend_type };

    // --- Build router ---
    let metrics_handle = init_metrics();
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app =
        create_router(state).merge(metrics_router).layer(agentd_common::server::trace_layer());

    // --- Bind and serve ---
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17005".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Wrap API server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
