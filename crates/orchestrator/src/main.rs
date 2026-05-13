mod api;
mod approvals;
mod config;
mod entity;
mod manager;
mod message_bridge;
mod migration;
mod scheduler;
mod skills;
mod storage;
mod system_agents;
mod types;
mod websocket;

use agentd_common::config::ValidateConfig;
use api::{create_router, ApiState};
use axum::{extract::State, response::IntoResponse, routing::get};
use communicate::client::CommunicateClient;
use communicate::error::CommunicateError;
use communicate::types::{
    AddParticipantRequest, CreateRoomRequest, ParticipantKind, ParticipantRole, RoomType,
};
use config::OrchestratorConfig;
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
use wrap::pty::PtyBackend;
use wrap::subprocess::SubprocessBackend;
use wrap::types::BackendType;

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

    let cfg = OrchestratorConfig::load();
    cfg.validate()?;

    // Initialize storage.
    let storage = AgentStorage::new().await?;
    info!("Agent storage initialized at: {:?}", AgentStorage::get_db_path()?);
    let storage = Arc::new(storage);

    // Determine the port and WS base URL early — both the Docker backend
    // and the AgentManager need it.
    let port_num: u16 = cfg.port;
    let ws_base_url = format!("ws://127.0.0.1:{}", cfg.port);

    // Execution backend — selected via AGENTD_BACKEND env var.
    // Valid values: "tmux" (default), "docker", "pty".
    // Unrecognised values cause an immediate startup failure.
    let backend_type = BackendType::from_env_strict()?;
    info!("Using execution backend: {}", backend_type);
    let backend: Arc<dyn ExecutionBackend> = match &backend_type {
        BackendType::Tmux => {
            info!("Using tmux execution backend");
            Arc::new(TmuxBackend::new("agentd-orch"))
        }
        BackendType::Docker => {
            let image = cfg.docker_image.as_deref().unwrap_or(wrap::docker::DEFAULT_IMAGE);
            info!(image = %image, "Using Docker execution backend");

            let docker_backend = DockerBackend::new("agentd-orch", image)
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
        BackendType::Pty => {
            info!("Using PTY execution backend");
            Arc::new(PtyBackend::new("agentd-orch"))
        }
        BackendType::Subprocess => {
            info!("Using subprocess execution backend");
            Arc::new(SubprocessBackend::new("agentd-orch"))
        }
    };

    // Shared event bus for internal lifecycle events.
    let event_bus = EventBus::shared(256);

    // WebSocket connection registry — attach storage so conversation events are
    // persisted as they flow through the WebSocket handler.
    let registry = ConnectionRegistry::new()
        .with_event_bus(event_bus.clone())
        .with_storage((*storage).clone());

    // Conversation event retention policy (read once at startup).
    let retention_config = Arc::new(types::RetentionConfig::from_env());
    info!(
        retention_days = retention_config.retention_days,
        max_events_per_agent = retention_config.max_events_per_agent,
        cleanup_on_terminate = retention_config.cleanup_on_terminate,
        cleanup_interval_secs = retention_config.cleanup_interval_secs,
        "Conversation retention config loaded",
    );

    // Agent manager (Arc'd immediately so it can be shared with callbacks and API state).
    let manager = Arc::new(AgentManager::with_retention(
        storage.clone(),
        backend,
        registry.clone(),
        ws_base_url,
        retention_config.clone(),
    ));

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

                    // 1. Persist usage to DB and emit cost metric.
                    // The snapshot contains cumulative session totals, so SET
                    // the gauge rather than incrementing it.
                    metrics::gauge!("usage_session_cost_usd_total").set(usage.total_cost_usd);

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
                            metrics::counter!("context_clears_total", "trigger" => "auto")
                                .increment(1);
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
        let bridge = Arc::new(message_bridge::MessageBridge::new(
            registry.clone(),
            communicate.clone(),
            storage.clone(),
            event_bus.clone(),
            &cfg.communicate_url,
        ));
        bridge.start().await;
        info!("MessageBridge started (communicate service: {})", cfg.communicate_url);
    }

    // Initialize Prometheus metrics
    let metrics_handle = init_metrics();

    // Build router with metrics endpoint and request tracing middleware.
    let state = ApiState {
        manager: manager.clone(),
        registry,
        scheduler: scheduler.clone(),
        communicate: communicate.clone(),
        backend_type,
    };
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let app = create_router(state)
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    // Bind and start serving BEFORE reconciliation. Reconcile restarts agent
    // processes that connect back to our WebSocket endpoint — the server must
    // be accepting connections before those agents are launched.
    let addr = format!("{}:{}", cfg.host, cfg.port);
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

    // Bootstrap built-in system agents after reconciliation so that surviving
    // user agents are handled first, and the system agent session URL is valid.
    if let Err(e) = manager.bootstrap_system_agents().await {
        error!(%e, "System agent bootstrap failed");
    }

    // Periodic reconciliation: detect agents whose processes died after startup.
    {
        let manager_reconcile = manager.clone();
        let interval_secs = cfg.reconcile_interval_secs;
        info!(interval_secs, "Starting periodic reconciliation loop");
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            interval.tick().await; // skip immediate tick (just ran reconcile above)
            loop {
                interval.tick().await;
                if let Err(e) = manager_reconcile.reconcile().await {
                    error!(%e, "Periodic reconciliation failed");
                }
            }
        });
    }

    // Periodic conversation event cleanup: prune old events and enforce the
    // per-agent cap on a configurable interval (default 6 h).
    {
        let storage_cleanup = storage.clone();
        let retention = retention_config.clone();
        let cleanup_interval = retention.cleanup_interval_secs;
        info!(
            cleanup_interval_secs = cleanup_interval,
            "Starting periodic conversation cleanup loop"
        );
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(cleanup_interval));
            interval.tick().await; // skip immediate tick on startup
            loop {
                interval.tick().await;

                // Age-based pruning: remove events older than retention_days.
                match storage_cleanup.prune_old_conversation_events(retention.retention_days).await
                {
                    Ok(n) if n > 0 => {
                        info!(pruned = n, "Pruned old conversation events");
                        metrics::counter!("agentd_conversation_events_pruned_total").increment(n);
                    }
                    Ok(_) => {}
                    Err(e) => error!(%e, "Conversation event age-prune failed"),
                }

                // Per-agent cap: enforce max_events_per_agent for every known agent.
                match storage_cleanup.list(None, None).await {
                    Ok(agents) => {
                        for agent in &agents {
                            match storage_cleanup
                                .prune_excess_conversation_events(
                                    agent.id,
                                    retention.max_events_per_agent,
                                )
                                .await
                            {
                                Ok(n) if n > 0 => {
                                    info!(
                                        agent_id = %agent.id,
                                        pruned = n,
                                        "Pruned excess conversation events for agent",
                                    );
                                    metrics::counter!("agentd_conversation_events_pruned_total")
                                        .increment(n);
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    error!(
                                        agent_id = %agent.id,
                                        %e,
                                        "Conversation event cap-prune failed for agent",
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => error!(%e, "Failed to list agents for per-agent cap pruning"),
                }
            }
        });
    }

    // Reactive disconnect handler: when an agent's WebSocket closes, check
    // whether its backend session is still alive after a short grace period.
    {
        let mut disconnect_rx = event_bus.subscribe();
        let manager_disconnect = manager.clone();
        tokio::spawn(async move {
            loop {
                match disconnect_rx.recv().await {
                    Ok(scheduler::events::SystemEvent::AgentDisconnected { agent_id }) => {
                        let mgr = manager_disconnect.clone();
                        tokio::spawn(async move {
                            // Grace period: allow intentional restarts to re-register
                            // before we mark the agent as failed.
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                            if let Err(e) = mgr.reconcile_single(&agent_id).await {
                                error!(%agent_id, %e, "Reactive reconcile after disconnect failed");
                            }
                        });
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Disconnect handler lagged by {} events", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    // Resume any enabled workflows from the database.
    if let Err(e) = scheduler.resume_workflows().await {
        error!(%e, "Failed to resume workflows");
    }
    let active_wf = scheduler.running_workflows().await.len();
    metrics::gauge!("workflows_active").set(active_wf as f64);

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

/// Returns the identifiers of stale name-based participant entries that should
/// be removed before an agent joins with its UUID.
///
/// A participant is considered stale when:
/// - Its `kind` is `Agent` (human participants legitimately use non-UUID names), and
/// - Its `identifier` equals the agent's name string.
///
/// This is extracted as a pure function so it can be unit-tested without a live
/// communicate service.
fn stale_name_based_identifiers<'a>(
    participants: &'a [communicate::types::ParticipantResponse],
    agent_name: &str,
) -> Vec<&'a str> {
    participants
        .iter()
        .filter(|p| p.kind == ParticipantKind::Agent && p.identifier == agent_name)
        .map(|p| p.identifier.as_str())
        .collect()
}

/// Look up a room by name via the communicate service, creating it if it doesn't
/// exist, then add the agent as a participant.
///
/// Returns the room UUID on success. Treat duplicate-participant errors as success.
///
/// # Duplicate reconciliation
///
/// Room templates applied via `agentd apply` add participants using the agent
/// **name** as the identifier (e.g. `"conductor"`), because the agent UUID is
/// not yet known at apply time.  When the agent later connects, the orchestrator
/// uses the **UUID** as the canonical identifier.  Both inserts succeed (they
/// are different strings), producing a duplicate row.
///
/// To prevent this, after finding/creating the room this function lists the
/// current participants and removes any entry whose identifier is *not* a valid
/// UUID but equals the agent's name — i.e., the stale name-based row left by
/// `agentd apply`.  Human participants (kind `human`) are never touched.
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
                    project_id: None,
                })
                .await?
        }
    };

    // Reconcile: remove stale name-based participant entries for this agent.
    //
    // `agentd apply` seeds rooms with participants identified by agent *name*
    // (e.g. "conductor") because the UUID is not yet known.  On first connect
    // the orchestrator adds the agent by UUID, creating a duplicate unless we
    // clean up the name-based row first.
    //
    // We only remove entries that:
    //   1. Have kind == Agent (human entries use non-UUID names legitimately), and
    //   2. Have an identifier equal to the agent's name (not a UUID).
    //
    // Failures here are non-fatal: the agent can still join; we just log and
    // continue so as not to block agent startup.
    match client.list_participants(room.id, 500, 0).await {
        Ok(participants) => {
            for identifier in stale_name_based_identifiers(&participants, agent_name) {
                match client.remove_participant(room.id, identifier).await {
                    Ok(_) => {
                        info!(
                            room_name,
                            agent_name,
                            identifier,
                            "Removed stale name-based participant entry; \
                             agent will rejoin with UUID identifier"
                        );
                    }
                    Err(CommunicateError::NotFound) => {
                        // Already removed by a concurrent call — ignore.
                    }
                    Err(e) => {
                        warn!(
                            room_name,
                            agent_name,
                            identifier,
                            err = ?e,
                            "Failed to remove stale name-based participant; \
                             duplicate may persist until next reconciliation"
                        );
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                room_name,
                agent_name,
                err = ?e,
                "Could not list participants for duplicate reconciliation; \
                 skipping cleanup"
            );
        }
    }

    // Add the agent as a participant using its UUID — treat 409 Conflict as
    // success (the agent was already a member from a previous session).
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
        Ok(_) | Err(CommunicateError::Conflict) => Ok(room.id),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use communicate::types::{ParticipantKind, ParticipantResponse, ParticipantRole};
    use uuid::Uuid;

    fn make_participant(identifier: &str, kind: ParticipantKind) -> ParticipantResponse {
        ParticipantResponse {
            id: Uuid::new_v4(),
            room_id: Uuid::new_v4(),
            identifier: identifier.to_string(),
            kind,
            display_name: identifier.to_string(),
            role: ParticipantRole::Member,
            joined_at: Utc::now(),
        }
    }

    #[test]
    fn stale_name_based_identifiers_returns_matching_agent_entry() {
        let uuid_str = Uuid::new_v4().to_string();
        let participants = vec![
            make_participant("conductor", ParticipantKind::Agent), // stale name-based
            make_participant(&uuid_str, ParticipantKind::Agent),   // canonical UUID entry
            make_participant("geoff", ParticipantKind::Human),     // human — never touched
        ];
        let stale = stale_name_based_identifiers(&participants, "conductor");
        assert_eq!(stale, vec!["conductor"]);
    }

    #[test]
    fn stale_name_based_identifiers_empty_when_no_match() {
        let uuid_str = Uuid::new_v4().to_string();
        let participants = vec![
            make_participant(&uuid_str, ParticipantKind::Agent),
            make_participant("geoff", ParticipantKind::Human),
        ];
        // No name-based entry for "conductor".
        let stale = stale_name_based_identifiers(&participants, "conductor");
        assert!(stale.is_empty());
    }

    #[test]
    fn stale_name_based_identifiers_ignores_human_with_same_name() {
        // Edge case: a human participant whose username happens to equal the
        // agent name.  Human entries must never be removed.
        let participants = vec![make_participant("conductor", ParticipantKind::Human)];
        let stale = stale_name_based_identifiers(&participants, "conductor");
        assert!(stale.is_empty(), "human entries should never be flagged as stale");
    }

    #[test]
    fn stale_name_based_identifiers_empty_room() {
        let stale = stale_name_based_identifiers(&[], "conductor");
        assert!(stale.is_empty());
    }

    #[test]
    fn stale_name_based_identifiers_concurrent_removal_guard() {
        // Verify that returning multiple stale entries (degenerate: duplicate
        // name rows) doesn't panic — the caller removes them one by one,
        // treating 404 (already removed) as success.
        let participants = vec![
            make_participant("conductor", ParticipantKind::Agent),
            make_participant("conductor", ParticipantKind::Agent),
        ];
        let stale = stale_name_based_identifiers(&participants, "conductor");
        assert_eq!(stale.len(), 2);
    }
}
