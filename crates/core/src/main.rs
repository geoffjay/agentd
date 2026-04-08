//! Core service entry point.
//!
//! Central authentication and API gateway service for agentd.
//!
//! # Environment Variables
//!
//! | Variable          | Default   | Description             |
//! |-------------------|-----------|-------------------------|
//! | `AGENTD_PORT`     | `17007`   | HTTP listen port        |
//! | `RUST_LOG`        | `info`    | Log level / filter      |
//! | `AGENTD_LOG_FORMAT` | (text)  | Set to `json` for JSON  |
//!
//! Note: port 17007 was chosen because 17010 (specified in issue #212) is
//! already used by the communicate service.

mod api;

use axum::{extract::State, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::future::IntoFuture;
use tracing::info;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "service" => "core"
    )
    .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    info!("Shutdown signal received");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-core service...");

    let db_path = agentd_common::storage::get_db_path("agentd-core", "core.db")?;
    let db = agentd_common::storage::create_connection(&db_path).await?;
    let _storage = agentd_core::storage::Storage::new(db).await?;
    info!("Database migrations applied");

    let metrics_handle = init_metrics();

    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = api::create_router()
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let port = std::env::var("AGENTD_PORT").unwrap_or_else(|_| "17007".to_string());
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Core API listening on http://{}", addr);

    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).into_future().await?;

    info!("Core service shut down");
    Ok(())
}
