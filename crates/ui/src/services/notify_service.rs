use anyhow::Result;
use notify::client::NotifyClient;
use notify::types::Notification;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manager for the notify service connection
#[derive(Clone)]
pub struct NotifyServiceManager {
    client: Arc<RwLock<Option<NotifyClient>>>,
}

impl NotifyServiceManager {
    /// Create a new unconnected service manager
    pub fn new() -> Self {
        Self { client: Arc::new(RwLock::new(None)) }
    }

    /// Connect to the notify service at the given URL
    pub async fn connect(&self, service_url: &str) -> Result<()> {
        let client = NotifyClient::new(service_url);

        // Test the connection with a health check
        client.health().await?;

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
        client.list_notifications().await
    }

    /// List actionable notifications (pending and requiring response)
    pub async fn list_actionable_notifications(&self) -> Result<Vec<Notification>> {
        let client_lock = self.client.read().await;
        let client = client_lock.as_ref().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        client.list_actionable_notifications().await
    }
}
