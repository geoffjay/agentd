//! HTTP client for interacting with the notification service.
//!
//! This module provides a strongly-typed client for making requests to the
//! notification service REST API. It handles serialization/deserialization
//! and provides ergonomic methods for all notification operations.
//!
//! # Examples
//!
//! ## Creating a client and listing notifications
//!
//! ```no_run
//! use notify::client::NotifyClient;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = NotifyClient::new("http://localhost:7004");
//! let notifications = client.list_notifications().await?;
//! println!("Found {} notifications", notifications.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Creating a notification
//!
//! ```no_run
//! use notify::client::NotifyClient;
//! use notify::types::*;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = NotifyClient::new("http://localhost:7004");
//!
//! let request = CreateNotificationRequest {
//!     source: NotificationSource::System,
//!     lifetime: NotificationLifetime::Persistent,
//!     priority: NotificationPriority::High,
//!     title: "Test".to_string(),
//!     message: "Test message".to_string(),
//!     requires_response: false,
//! };
//!
//! let notification = client.create_notification(&request).await?;
//! println!("Created notification: {}", notification.id);
//! # Ok(())
//! # }
//! ```

use crate::types::*;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

/// Client for the notification service REST API.
///
/// Provides strongly-typed methods for all notification operations including
/// creating, listing, updating, and deleting notifications.
///
/// # Examples
///
/// ```
/// use notify::client::NotifyClient;
///
/// let client = NotifyClient::new("http://localhost:7004");
/// ```
#[derive(Clone)]
pub struct NotifyClient {
    client: reqwest::Client,
    base_url: String,
}

impl NotifyClient {
    /// Create a new notification service client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the notification service (e.g., "http://localhost:7004")
    ///
    /// # Examples
    ///
    /// ```
    /// use notify::client::NotifyClient;
    ///
    /// let client = NotifyClient::new("http://localhost:7004");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// List all notifications.
    ///
    /// Retrieves all notifications from the service, regardless of status.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let notifications = client.list_notifications().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_notifications(&self) -> Result<Vec<Notification>> {
        self.get("/notifications").await
    }

    /// List notifications filtered by status.
    ///
    /// # Arguments
    ///
    /// * `status` - The status to filter by
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # use notify::types::NotificationStatus;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let pending = client.list_notifications_by_status(NotificationStatus::Pending).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_notifications_by_status(
        &self,
        status: NotificationStatus,
    ) -> Result<Vec<Notification>> {
        let status_str = format!("{status:?}").to_lowercase();
        self.get(&format!("/notifications?status={status_str}")).await
    }

    /// List only actionable notifications.
    ///
    /// Returns notifications that are pending and require a response.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let actionable = client.list_actionable_notifications().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_actionable_notifications(&self) -> Result<Vec<Notification>> {
        self.get("/notifications/actionable").await
    }

    /// Get a specific notification by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the notification to retrieve
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    /// let notification = client.get_notification(id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_notification(&self, id: Uuid) -> Result<Notification> {
        self.get(&format!("/notifications/{id}")).await
    }

    /// Create a new notification.
    ///
    /// # Arguments
    ///
    /// * `request` - The notification creation request
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # use notify::types::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    ///
    /// let request = CreateNotificationRequest {
    ///     source: NotificationSource::System,
    ///     lifetime: NotificationLifetime::Persistent,
    ///     priority: NotificationPriority::Normal,
    ///     title: "Test".to_string(),
    ///     message: "Test message".to_string(),
    ///     requires_response: false,
    /// };
    ///
    /// let notification = client.create_notification(&request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_notification(
        &self,
        request: &CreateNotificationRequest,
    ) -> Result<Notification> {
        self.post("/notifications", request).await
    }

    /// Update an existing notification.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the notification to update
    /// * `request` - The notification update request
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # use notify::types::*;
    /// # use uuid::Uuid;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    ///
    /// let request = UpdateNotificationRequest {
    ///     status: Some(NotificationStatus::Viewed),
    ///     response: None,
    /// };
    ///
    /// let notification = client.update_notification(id, &request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_notification(
        &self,
        id: Uuid,
        request: &UpdateNotificationRequest,
    ) -> Result<Notification> {
        self.put(&format!("/notifications/{id}"), request).await
    }

    /// Delete a notification.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the notification to delete
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    /// client.delete_notification(id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_notification(&self, id: Uuid) -> Result<()> {
        self.delete(&format!("/notifications/{id}")).await
    }

    /// Get notification counts grouped by status.
    ///
    /// Returns statistics about how many notifications exist in each status category.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// let counts = client.count_notifications().await?;
    /// println!("Total notifications: {}", counts.total);
    /// for status_count in counts.by_status {
    ///     println!("{}: {}", status_count.status, status_count.count);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn count_notifications(&self) -> Result<CountResponse> {
        self.get("/notifications/count").await
    }

    /// Check the health of the notification service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use notify::client::NotifyClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = NotifyClient::new("http://localhost:7004");
    /// client.health().await?;
    /// println!("Service is healthy");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health(&self) -> Result<()> {
        self.get::<serde_json::Value>("/health").await?;
        Ok(())
    }

    // Internal helper methods

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context(format!("Failed to GET {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn put<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to PUT {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        response.json().await.context("Failed to parse response JSON")
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.delete(&url).send().await.context(format!("Failed to DELETE {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {status}: {body}");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = NotifyClient::new("http://localhost:7004");
        assert_eq!(client.base_url, "http://localhost:7004");
    }

    #[test]
    fn test_client_creation_with_string() {
        let url = String::from("http://localhost:7004");
        let client = NotifyClient::new(url);
        assert_eq!(client.base_url, "http://localhost:7004");
    }

    #[test]
    fn test_client_clone() {
        let client1 = NotifyClient::new("http://localhost:7004");
        let client2 = client1.clone();
        assert_eq!(client1.base_url, client2.base_url);
    }
}
