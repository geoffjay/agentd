mod api;
mod approvals;
mod entity;
mod manager;
mod message_bridge;
mod migration;
mod scheduler;
mod storage;
mod types;
mod websocket;

use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use communicate::client::CommunicateClient;
use communicate::types::{
    AddParticipantRequest, CreateRoomRequest, ParticipantKind, ParticipantRole, RoomType,
};
use manager::AgentManager;
use metrics_exporter_prometheus::PrometheusHandle;
use scheduler::events::EventBus;
use scheduler::storage::SchedulerStorage;
use scheduler::Scheduler;
use std::collections::HashSet;
use std::env;
use std::future::IntoFuture;
use std::sync::Arc;
use storage::AgentStorage;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;
use websocket::ConnectionRegistry;
use wrap::backend::{ExecutionBackend, TmuxBackend};
use wrap::docker::DockerBackend;

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

    // Determine the port and WS base URL early — both the Docker backend
    // and the AgentManager need it.
    let port = env::var("AGENTD_PORT").unwrap_or_else(|_| "17006".to_string());
    let port_num: u16 = port.parse().expect("AGENTD_PORT must be a valid u16");
    let ws_base_url = format!("ws://127.0.0.1:{}", port);

    // Execution backend — selected via AGENTD_BACKEND env var.
    // Valid values: "tmux" (default), "docker".
    let backend_mode = env::var("AGENTD_BACKEND").unwrap_or_else(|_| "tmux".to_string());
    let backend: Arc<dyn ExecutionBackend> = match backend_mode.as_str() {
        "tmux" => {
            info!("Using tmux execution backend");
            Arc::new(TmuxBackend::new("agentd-orch"))
        }
        "docker" => {
            let image = env::var("AGENTD_DOCKER_IMAGE")
                .unwrap_or_else(|_| wrap::docker::DEFAULT_IMAGE.to_string());
            info!(image = %image, "Using Docker execution backend");

            let docker_backend = DockerBackend::new("agentd-orch", &image)
                .map_err(|e| anyhow::anyhow!("Failed to initialize Docker backend: {}", e))?
                .with_orchestrator_port(port_num);

            // Validate that the Docker daemon is reachable before proceeding.
            // A simple `list_sessions` call exercises the Docker API.
            docker_backend.list_sessions().await.map_err(|e| {
                anyhow::anyhow!(
                    "Docker daemon is unreachable (AGENTD_BACKEND=docker). \
                     Ensure Docker is running and accessible: {}",
                    e
                )
            })?;
            info!("Docker daemon connectivity verified");

            Arc::new(docker_backend)
        }
        other => {
            anyhow::bail!("Unknown AGENTD_BACKEND value '{}'. Valid options: tmux, docker", other);
        }
    };

    // Shared event bus for internal lifecycle events.
    let event_bus = EventBus::shared(256);

    // WebSocket connection registry.
    let registry = ConnectionRegistry::new().with_event_bus(event_bus.clone());

    // Agent manager (Arc'd immediately so it can be shared with callbacks and API state).
    let manager =
        Arc::new(AgentManager::new(storage.clone(), backend, registry.clone(), ws_base_url));

    // Scheduler for autonomous workflows (shares the same SeaORM connection).
    // Schema is already applied by AgentStorage::with_path() via Migrator::up().
    let scheduler_storage = SchedulerStorage::new(storage.db().clone());
    let scheduler = Arc::new(
        Scheduler::new(scheduler_storage, registry.clone()).with_event_bus(event_bus.clone()),
    );

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
                    if let Err(e) = storage.record_session_usage(&info.agent_id, &usage).await {
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

                    let current_input =
                        stats.current_session.as_ref().map(|s| s.input_tokens).unwrap_or(0);

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

    // Single shared communicate client used by both the room auto-join task
    // and the MessageBridge below.
    let communicate = CommunicateClient::from_env();

    // Spawn a task that auto-joins agents to their configured rooms on connect.
    //
    // Subscribes to the event bus and reacts to `AgentConnected` events.
    // Errors from the communicate service are logged as warnings and never
    // prevent the agent from starting up.
    {
        let mut event_rx = event_bus.subscribe();
        let manager = manager.clone();
        let communicate = communicate.clone();
        let bus = event_bus.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(scheduler::events::SystemEvent::AgentConnected { agent_id }) => {
                        let agent = match manager.get_agent(&agent_id).await {
                            Ok(Some(a)) => a,
                            Ok(None) => continue,
                            Err(e) => {
                                warn!(%agent_id, %e, "Failed to look up agent for room auto-join");
                                continue;
                            }
                        };
                        if agent.config.rooms.is_empty() {
                            continue;
                        }
                        let communicate = communicate.clone();
                        let agent_name = agent.name.clone();
                        let rooms = agent.config.rooms.clone();
                        let bus = bus.clone();
                        tokio::spawn(async move {
                            for room_name in &rooms {
                                match join_or_create_room(
                                    &communicate,
                                    &agent_id,
                                    &agent_name,
                                    room_name,
                                )
                                .await
                                {
                                    Ok(room_id) => {
                                        info!(%agent_id, %room_name, %room_id, "Agent joined room");
                                        bus.publish(
                                            scheduler::events::SystemEvent::AgentJoinedRoom {
                                                agent_id,
                                                room_id,
                                            },
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            %agent_id,
                                            %room_name,
                                            error = %e,
                                            "Failed to auto-join room (communicate service may be unavailable)"
                                        );
                                    }
                                }
                            }
                        });
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Room auto-join task lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Start the message bridge: connects to the communicate service and routes
    // room messages to agent prompts (and agent responses back to rooms).
    // start() spawns background tasks and returns immediately — it does not
    // block the orchestrator startup even if the communicate service is slow.
    {
        let communicate_url = env::var("AGENTD_COMMUNICATE_SERVICE_URL")
            .unwrap_or_else(|_| "http://localhost:17010".to_string());
        let bridge = Arc::new(message_bridge::MessageBridge::new(
            registry.clone(),
            communicate.clone(),
            storage.clone(),
            event_bus.clone(),
            &communicate_url,
        ));
        bridge.start().await;
        info!("MessageBridge started (communicate service: {})", communicate_url);
    }

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Build router with metrics endpoint and request tracing middleware.
    let state = ApiState {
        manager: manager.clone(),
        registry,
        scheduler: scheduler.clone(),
        communicate: communicate.clone(),
    };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(state)
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    // Bind and start serving BEFORE reconciliation. Reconcile restarts agent
    // processes that connect back to our WebSocket endpoint — the server must
    // be accepting connections before those agents are launched.
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Orchestrator API listening on http://{}", addr);
    info!("WebSocket endpoint at ws://{}/ws/{{agent_id}}", addr);

    let server = tokio::spawn(
        axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).into_future(),
    );

    // Now that the server is accepting connections, reconcile stale agents.
    // Restarted Claude processes will connect to the already-listening WebSocket
    // endpoint instead of failing because the port isn't bound yet.
    if let Err(e) = manager.reconcile().await {
        error!(%e, "Agent reconciliation failed");
    }

    // Resume any enabled workflows from the database.
    if let Err(e) = scheduler.resume_workflows().await {
        error!(%e, "Failed to resume workflows");
    }

    server.await??;

    // Graceful shutdown: stop all managed agent sessions.
    // AGENTD_SHUTDOWN_LEAVE_RUNNING=true leaves sessions alive for reconnection.
    let leave_running =
        env::var("AGENTD_SHUTDOWN_LEAVE_RUNNING").map(|v| v == "true" || v == "1").unwrap_or(false);
    manager.shutdown_all(leave_running).await;

    // Graceful shutdown: stop all workflow runners.
    scheduler.shutdown_all().await;

    info!("Orchestrator service shut down");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("Failed to install ctrl+c handler");
    info!("Shutdown signal received");
}

/// Look up a room by name via the communicate service, creating it if it doesn't
/// exist, then add the agent as a participant.
///
/// Returns the room UUID on success. Treat duplicate-participant errors as success.
async fn join_or_create_room(
    client: &CommunicateClient,
    agent_id: &Uuid,
    agent_name: &str,
    room_name: &str,
) -> anyhow::Result<Uuid> {
    // Find or create the room.
    let room = match client.get_room_by_name(room_name).await? {
        Some(room) => room,
        None => {
            client
                .create_room(&CreateRoomRequest {
                    name: room_name.to_string(),
                    topic: None,
                    description: None,
                    room_type: RoomType::Group,
                    created_by: agent_name.to_string(),
                })
                .await?
        }
    };

    // Add the agent as a participant — treat 409 Conflict as success
    // (the agent is already a member).
    let identifier = agent_id.to_string();
    match client
        .add_participant(
            room.id,
            &AddParticipantRequest {
                identifier,
                kind: ParticipantKind::Agent,
                display_name: agent_name.to_string(),
                role: ParticipantRole::Member,
            },
        )
        .await
    {
        Ok(_) | Err(communicate::error::CommunicateError::Conflict) => Ok(room.id),
        Err(e) => Err(e.into()),
    }
}
