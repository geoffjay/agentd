mod api;
mod approvals;
mod entity;
mod manager;
mod migration;
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
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use storage::AgentStorage;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;
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

    // Scheduler for autonomous workflows (shares the same SeaORM connection).
    // Schema is already applied by AgentStorage::with_path() via Migrator::up().
    let scheduler_storage = SchedulerStorage::new(storage.db().clone());
    let scheduler = Arc::new(Scheduler::new(scheduler_storage, registry.clone()));

    // Register scheduler as a result callback so it gets notified when agents finish.
    {
        let sched = scheduler.clone();
        registry
            .on_result(Arc::new(move |info| {
                let sched = sched.clone();
                tokio::spawn(async move {
                    sched.notify_task_complete(info.agent_id, info.is_error).await;
                });
            }))
            .await;
    }

    // Register usage persistence and auto-clear callback.
    {
        let storage = storage.clone();
        let manager = manager.clone();
        let clearing = Arc::new(RwLock::new(HashSet::<Uuid>::new()));
        registry
            .on_result(Arc::new(move |info| {
                let storage = storage.clone();
                let manager = manager.clone();
                let clearing = clearing.clone();
                tokio::spawn(async move {
                    // Skip results without usage data.
                    let usage = match info.usage {
                        Some(ref u) => u.clone(),
                        None => return,
                    };

                    // 1. Persist usage to DB.
                    if let Err(e) =
                        storage.record_session_usage(&info.agent_id, &usage).await
                    {
                        error!(
                            agent_id = %info.agent_id,
                            %e,
                            "Failed to persist usage data"
                        );
                        return;
                    }

                    // 2. Check auto-clear threshold.
                    let agent = match storage.get(&info.agent_id).await {
                        Ok(Some(a)) => a,
                        Ok(None) => return,
                        Err(e) => {
                            error!(
                                agent_id = %info.agent_id,
                                %e,
                                "Failed to look up agent for auto-clear check"
                            );
                            return;
                        }
                    };

                    let threshold = match agent.config.auto_clear_threshold {
                        Some(t) => t,
                        None => return,
                    };

                    // Get current session stats to check accumulated input_tokens.
                    let stats = match storage.get_usage_stats(&info.agent_id).await {
                        Ok(s) => s,
                        Err(e) => {
                            error!(
                                agent_id = %info.agent_id,
                                %e,
                                "Failed to get usage stats for auto-clear check"
                            );
                            return;
                        }
                    };

                    let current_input = stats
                        .current_session
                        .as_ref()
                        .map(|s| s.input_tokens)
                        .unwrap_or(0);

                    if current_input < threshold {
                        return;
                    }

                    // 3. Re-entrancy guard: prevent concurrent auto-clears for
                    //    the same agent.
                    {
                        let mut guard = clearing.write().await;
                        if guard.contains(&info.agent_id) {
                            return;
                        }
                        guard.insert(info.agent_id);
                    }

                    info!(
                        agent_id = %info.agent_id,
                        current_input,
                        threshold,
                        "Auto-clearing agent context (threshold exceeded)"
                    );

                    match manager.clear_context(&info.agent_id).await {
                        Ok(resp) => {
                            info!(
                                agent_id = %info.agent_id,
                                new_session = resp.new_session_number,
                                "Auto-clear completed"
                            );
                        }
                        Err(e) => {
                            error!(
                                agent_id = %info.agent_id,
                                %e,
                                "Auto-clear failed"
                            );
                        }
                    }

                    // Always remove from clearing set (success or error).
                    clearing.write().await.remove(&info.agent_id);
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

    let app = create_router(state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

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
