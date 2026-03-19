//! agentd-communicate service entry point.
//!
//! REST API on `http://127.0.0.1:17010` (dev default, `AGENTD_PORT` to override).
//! SQLite database at platform-specific path (`agentd-communicate/communicate.db`).

mod api;
mod entity;
mod migration;
mod storage;
mod types;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::env;
use std::sync::Arc;
use storage::CommunicateStorage;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!(
        "service_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "service" => "communicate"
    )
    .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-communicate service...");

    let storage = CommunicateStorage::new().await?;
    info!("Communicate storage initialized at: {:?}", CommunicateStorage::get_db_path()?);

    let storage = Arc::new(storage);
    let metrics_handle = init_metrics();

    let api_state = ApiState { storage };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(api_state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17010".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Communicate API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
