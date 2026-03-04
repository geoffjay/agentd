mod api;
mod approvals;
mod manager;
mod scheduler;
mod storage;
mod types;
mod websocket;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use manager::AgentManager;
use metrics_exporter_prometheus::PrometheusHandle;
use scheduler::storage::SchedulerStorage;
use scheduler::Scheduler;
use std::env;
use std::sync::Arc;
use storage::AgentStorage;
use tracing::info;
use websocket::ConnectionRegistry;
use wrap::tmux::TmuxManager;

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "orchestrator")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agentd_common::server::init_tracing();

    info!("Starting agentd-orchestrator service...");

    // Initialize storage.
    let storage = AgentStorage::new().await?;
    info!("Agent storage initialized at: {:?}", AgentStorage::get_db_path()?);
    let storage = Arc::new(storage);

    // Tmux manager with orchestrator-specific prefix.
    let tmux = TmuxManager::new("agentd-orch");

    // WebSocket connection registry.
    let registry = ConnectionRegistry::new();

    // Determine the port and WS base URL.
    let port = env::var("PORT").unwrap_or_else(|_| "17006".to_string());
    let ws_base_url = format!("ws://127.0.0.1:{}", port);

    // Agent manager.
    let manager = AgentManager::new(storage.clone(), tmux, registry.clone(), ws_base_url);

    // Reconcile DB state with tmux sessions from any previous run.
    manager.reconcile().await?;

    let manager = Arc::new(manager);

    // Scheduler for autonomous workflows (shares the same SQLite pool).
    let scheduler_storage = SchedulerStorage::new(storage.pool().clone());
    scheduler_storage.init_schema().await?;
    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));

    // Register scheduler as a result callback so it gets notified when agents finish.
    {
        let sched = scheduler.clone();
        registry
            .on_result(Arc::new(move |agent_id, is_error| {
                let sched = sched.clone();
                tokio::spawn(async move {
                    sched.notify_task_complete(agent_id, is_error).await;
                });
            }))
            .await;
    }

    // Resume any enabled workflows from the database.
    scheduler.resume_workflows().await?;

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Build router with metrics endpoint and request tracing middleware.
    let state = ApiState { manager, registry, scheduler: scheduler.clone() };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app =
        create_router(state).merge(metrics_router).layer(agentd_common::server::trace_layer());

    // Bind and serve.
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Orchestrator API listening on http://{}", addr);
    info!("WebSocket endpoint at ws://{}/ws/{{agent_id}}", addr);

    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;

    // Graceful shutdown: stop all workflow runners.
    scheduler.shutdown_all().await;

    info!("Orchestrator service shut down");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("Failed to install ctrl+c handler");
    info!("Shutdown signal received");
}
