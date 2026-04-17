//! agentd-wrap service entry point.
//!
//! Provides a REST API for launching and managing agent sessions. The active
//! execution backend is selected via the `AGENTD_BACKEND` environment variable:
//!
//! | Value        | Backend                      |
//! |--------------|------------------------------|
//! | `tmux`       | tmux sessions (default)      |
//! | `docker`     | Docker containers            |
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
//! - `AGENTD_BACKEND`               — Execution backend: `tmux` | `docker` | `pty` | `subprocess` (default: `tmux`)
//! - `AGENTD_WRAP_HISTORY_BYTES`    — PTY output ring-buffer size in bytes (default: `524288` / 512 KiB); agentd-wrap PTY backend only
//! - `AGENTD_WRAP_CHANNEL_CAPACITY` — PTY broadcast channel capacity in chunks (default: `256`); must be ≥ 1; agentd-wrap PTY backend only

// Import from the library target — avoids re-declaring modules in the binary and
// triggering dead-code warnings on items that are only used by the library.
use wrap::{
    api::{create_router, AppState},
    backend::{ExecutionBackend, TmuxBackend},
    docker::{DockerBackend, DEFAULT_IMAGE},
    pty::PtyBackend,
    pty_stream::{DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES},
    subprocess::SubprocessBackend,
    types::BackendType,
};

use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::{env, sync::Arc};
use tracing::{info, warn};

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
    // Unrecognised AGENTD_BACKEND values cause an immediate startup failure.
    let backend_type = BackendType::from_env_strict()?;
    info!("Using execution backend: {}", backend_type);

    let exec_backend: Arc<dyn ExecutionBackend> = match &backend_type {
        BackendType::Pty => {
            info!("Initialising PTY backend");
            let history_bytes = env::var("AGENTD_WRAP_HISTORY_BYTES")
                .ok()
                .and_then(|raw| {
                    raw.parse::<usize>()
                        .map_err(|_| {
                            warn!(
                                "AGENTD_WRAP_HISTORY_BYTES={:?} is not a valid usize; \
                             using default {} bytes",
                                raw, DEFAULT_HISTORY_BYTES
                            );
                        })
                        .ok()
                })
                .unwrap_or(DEFAULT_HISTORY_BYTES);
            let channel_capacity = {
                let parsed = env::var("AGENTD_WRAP_CHANNEL_CAPACITY")
                    .ok()
                    .and_then(|raw| {
                        raw.parse::<usize>()
                            .map_err(|_| {
                                warn!(
                                    "AGENTD_WRAP_CHANNEL_CAPACITY={:?} is not a valid usize; \
                                 using default {}",
                                    raw, DEFAULT_CHANNEL_CAPACITY
                                );
                            })
                            .ok()
                    })
                    .unwrap_or(DEFAULT_CHANNEL_CAPACITY);
                // broadcast::channel(0) panics — clamp to at least 1.
                if parsed == 0 {
                    warn!(
                        "AGENTD_WRAP_CHANNEL_CAPACITY=0 is invalid \
                         (tokio broadcast::channel requires capacity ≥ 1); clamped to 1"
                    );
                    1
                } else {
                    parsed
                }
            };
            info!(
                "PTY ring-buffer: history_bytes={}, channel_capacity={}",
                history_bytes, channel_capacity
            );
            Arc::new(PtyBackend::new_with_config("agentd", channel_capacity, history_bytes))
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
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17005".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Wrap API server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
