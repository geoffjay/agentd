use anyhow::Result;
use notify::client::NotifyClient;
use notify::types::Notification;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Manager for the notify service connection
#[derive(Clone)]
pub struct NotifyServiceManager {
    client: Arc<RwLock<Option<NotifyClient>>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl NotifyServiceManager {
    /// Create a new unconnected service manager
    pub fn new() -> Self {
        // Create a dedicated tokio runtime for HTTP requests
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        Self { client: Arc::new(RwLock::new(None)), runtime: Arc::new(runtime) }
    }

    /// Connect to the notify service at the given URL
    pub async fn connect(&self, service_url: &str) -> Result<()> {
        let service_url = service_url.to_string();
        let runtime = self.runtime.clone();

        // Run the HTTP call in the tokio runtime
        let result = runtime.spawn(async move {
            let client = NotifyClient::new(&service_url);
            // Test the connection with a health check
            client.health().await?;
            Ok::<_, anyhow::Error>(client)
        });

        let client = result.await??;
        let mut client_lock = self.client.write().await;
        *client_lock = Some(client);

        Ok(())
    }

    /// Disconnect from the notify service
    pub async fn disconnect(&self) {
        let mut client_lock = self.client.write().await;
        *client_lock = None;
    }

    /// Check if connected to a service
    pub async fn is_connected(&self) -> bool {
        let client_lock = self.client.read().await;
        client_lock.is_some()
    }

    /// List all notifications
    pub async fn list_notifications(&self) -> Result<Vec<Notification>> {
        let client_lock = self.client.read().await;
        let client = client_lock.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let client = client.clone();
        drop(client_lock);

        let runtime = self.runtime.clone();
        let result = runtime.spawn(async move { client.list_notifications().await });

        result.await?
    }

    /// List actionable notifications (pending and requiring response)
    pub async fn list_actionable_notifications(&self) -> Result<Vec<Notification>> {
        let client_lock = self.client.read().await;
        let client = client_lock.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let client = client.clone();
        drop(client_lock);

        let runtime = self.runtime.clone();
        let result = runtime.spawn(async move { client.list_actionable_notifications().await });

        result.await?
    }

    /// Delete a notification by ID
    pub async fn delete_notification(&self, id: Uuid) -> Result<()> {
        let client_lock = self.client.read().await;
        let client = client_lock.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let client = client.clone();
        drop(client_lock);

        let runtime = self.runtime.clone();
        let result = runtime.spawn(async move { client.delete_notification(id).await });

        result.await?
    }
}
