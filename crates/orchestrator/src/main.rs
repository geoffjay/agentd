mod api;
mod manager;
mod storage;
mod types;
mod websocket;

use api::{create_router, ApiState};
use manager::AgentManager;
use std::env;
use std::sync::Arc;
use storage::AgentStorage;
use tracing::info;
use websocket::ConnectionRegistry;
use wrap::tmux::TmuxManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

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
    let manager = AgentManager::new(storage, tmux, registry.clone(), ws_base_url);

    // Reconcile DB state with tmux sessions from any previous run.
    manager.reconcile().await?;

    let manager = Arc::new(manager);

    // Build router.
    let state = ApiState { manager, registry };
    let app = create_router(state);

    // Bind and serve.
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Orchestrator API listening on http://{}", addr);
    info!("WebSocket endpoint at ws://{}/ws/{{agent_id}}", addr);

    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;

    info!("Orchestrator service shut down");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("Failed to install ctrl+c handler");
    info!("Shutdown signal received");
}
