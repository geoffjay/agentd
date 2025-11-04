mod api;
mod notification;
mod storage;

use api::{create_router, ApiState};
use std::sync::Arc;
use storage::NotificationStorage;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting agentd-notify service...");

    // Initialize SQLite storage
    let storage = NotificationStorage::new().await?;
    info!(
        "Notification storage initialized at: {:?}",
        NotificationStorage::get_db_path()?
    );

    // Wrap storage for sharing
    let storage = Arc::new(storage);

    // Spawn background cleanup task
    let storage_clone = storage.clone();
    tokio::spawn(async move {
        let mut cleanup_interval = interval(Duration::from_secs(300)); // Every 5 minutes
        loop {
            cleanup_interval.tick().await;
            match storage_clone.cleanup_expired().await {
                Ok(count) => {
                    if count > 0 {
                        info!("Cleaned up {} expired notifications", count);
                    }
                }
                Err(e) => {
                    warn!("Failed to cleanup expired notifications: {}", e);
                }
            }
        }
    });

    // Create API state and router
    let api_state = ApiState {
        storage: storage.clone(),
    };
    let app = create_router(api_state);

    // Bind to address
    let addr = "127.0.0.1:3030";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Notification API server listening on http://{}", addr);

    // Start the HTTP server
    axum::serve(listener, app).await?;

    Ok(())
}
