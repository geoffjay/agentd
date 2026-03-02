//! HTTP client for the notification service.
//!
//! This module provides an HTTP client for communicating with the `agentd-notify`
//! service. It handles creating notifications, updating their status, and checking
//! service health.
//!
//! # Communication Protocol
//!
//! The client uses HTTP/REST to communicate with the notification service:
//! - `POST /notifications` - Create a new notification
//! - `PUT /notifications/{id}` - Update a notification
//! - `GET /notifications/{id}` - Retrieve a notification
//! - `GET /health` - Check service health
//!
//! # Error Handling
//!
//! All methods return [`Result<T, NotificationError>`] for consistent error handling.
//! Errors are categorized as:
//! - [`NotificationError::HttpError`] - Network/connection failures
//! - [`NotificationError::ServiceUnavailable`] - Service returned error status
//! - [`NotificationError::InvalidResponse`] - Response parsing failed
//!
//! # Examples
//!
//! ## Creating a client and checking health
//!
//! ```no_run
//! use ask::notification_client::NotificationClient;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = NotificationClient::new("http://localhost:17004".to_string());
//!
//! match client.health_check().await {
//!     Ok(true) => println!("Notification service is healthy"),
//!     Ok(false) => println!("Service returned unhealthy status"),
//!     Err(e) => eprintln!("Health check failed: {}", e),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Creating a notification
//!
//! ```no_run
//! use ask::notification_client::NotificationClient;
//! use uuid::Uuid;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = NotificationClient::new("http://localhost:17004".to_string());
//! let question_id = Uuid::new_v4();
//!
//! let notification = client.create_tmux_session_question(question_id).await?;
//! println!("Created notification: {}", notification.id);
//! # Ok(())
//! # }
//! ```

use crate::error::NotificationError;
use crate::types::{
    CreateNotificationRequest, Notification, NotificationLifetime, NotificationPriority,
    NotificationSource, UpdateNotificationRequest,
};
use reqwest::Client;
use tracing::{debug, error};
use uuid::Uuid;

/// HTTP client for the notification service.
///
/// Provides methods for creating, updating, and querying notifications via HTTP.
/// The client uses `reqwest` for HTTP communication and can be cloned cheaply.
///
/// # Thread Safety
///
/// The client is thread-safe and can be shared across multiple async tasks.
/// Cloning is cheap as it only clones the underlying `reqwest::Client` which
/// uses an `Arc` internally.
///
/// # Examples
///
/// ```no_run
/// use ask::notification_client::NotificationClient;
///
/// let client = NotificationClient::new("http://localhost:17004".to_string());
/// let client_clone = client.clone(); // Cheap clone
/// ```
#[derive(Clone)]
pub struct NotificationClient {
    client: Client,
    base_url: String,
}

impl NotificationClient {
    /// Creates a new notification client.
    ///
    /// # Arguments
    ///
    /// - `base_url` - The base URL of the notification service (e.g., "http://localhost:17004")
    ///
    /// # Returns
    ///
    /// Returns a new [`NotificationClient`] configured to communicate with the service.
    ///
    /// # Examples
    ///
    /// ```
    /// use ask::notification_client::NotificationClient;
    ///
    /// let client = NotificationClient::new("http://localhost:17004".to_string());
    /// ```
    pub fn new(base_url: String) -> Self {
        Self { client: Client::new(), base_url }
    }

    /// Creates a new notification via the notification service.
    ///
    /// Sends a POST request to `/notifications` with the notification details.
    ///
    /// # Arguments
    ///
    /// - `request` - The [`CreateNotificationRequest`] containing notification details
    ///
    /// # Returns
    ///
    /// Returns `Ok(Notification)` with the created notification including its ID,
    /// or an error if the request fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails ([`NotificationError::HttpError`])
    /// - Service returns non-success status ([`NotificationError::ServiceUnavailable`])
    /// - Response cannot be parsed ([`NotificationError::InvalidResponse`])
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ask::notification_client::NotificationClient;
    /// use ask::types::{
    ///     CreateNotificationRequest, NotificationSource, NotificationLifetime,
    ///     NotificationPriority,
    /// };
    /// use uuid::Uuid;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NotificationClient::new("http://localhost:17004".to_string());
    ///
    /// let request = CreateNotificationRequest {
    ///     source: NotificationSource::AskService { request_id: Uuid::new_v4() },
    ///     lifetime: NotificationLifetime::Persistent,
    ///     priority: NotificationPriority::Normal,
    ///     title: "Question".to_string(),
    ///     message: "Do you want to proceed?".to_string(),
    ///     requires_response: true,
    /// };
    ///
    /// let notification = client.create_notification(request).await?;
    /// println!("Created notification: {}", notification.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_notification(
        &self,
        request: CreateNotificationRequest,
    ) -> Result<Notification, NotificationError> {
        let url = format!("{}/notifications", self.base_url);

        debug!("Creating notification: {}", url);

        let response = self.client.post(&url).json(&request).send().await.map_err(|e| {
            error!("Failed to send notification request: {}", e);
            NotificationError::HttpError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Notification service returned error {}: {}", status, error_text);
            return Err(NotificationError::ServiceUnavailable(format!(
                "HTTP {status}: {error_text}"
            )));
        }

        let notification: Notification = response.json().await.map_err(|e| {
            error!("Failed to parse notification response: {}", e);
            NotificationError::InvalidResponse(e.to_string())
        })?;

        debug!("Created notification with ID: {}", notification.id);
        Ok(notification)
    }

    /// Updates an existing notification.
    ///
    /// Sends a PUT request to `/notifications/{id}` to update notification status
    /// and/or response text.
    ///
    /// # Arguments
    ///
    /// - `notification_id` - The UUID of the notification to update
    /// - `request` - The [`UpdateNotificationRequest`] containing updated fields
    ///
    /// # Returns
    ///
    /// Returns `Ok(Notification)` with the updated notification on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Network request fails ([`NotificationError::HttpError`])
    /// - Notification not found or service returns error ([`NotificationError::ServiceUnavailable`])
    /// - Response cannot be parsed ([`NotificationError::InvalidResponse`])
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ask::notification_client::NotificationClient;
    /// use ask::types::{UpdateNotificationRequest, NotificationStatus};
    /// use uuid::Uuid;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NotificationClient::new("http://localhost:17004".to_string());
    /// let notification_id = Uuid::new_v4();
    ///
    /// let request = UpdateNotificationRequest {
    ///     status: Some(NotificationStatus::Responded),
    ///     response: Some("yes".to_string()),
    /// };
    ///
    /// let notification = client.update_notification(notification_id, request).await?;
    /// println!("Updated notification status: {:?}", notification.status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_notification(
        &self,
        notification_id: Uuid,
        request: UpdateNotificationRequest,
    ) -> Result<Notification, NotificationError> {
        let url = format!("{}/notifications/{}", self.base_url, notification_id);

        debug!("Updating notification {}: {}", notification_id, url);

        let response = self.client.put(&url).json(&request).send().await.map_err(|e| {
            error!("Failed to send update request: {}", e);
            NotificationError::HttpError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Notification service returned error {}: {}", status, error_text);
            return Err(NotificationError::ServiceUnavailable(format!(
                "HTTP {status}: {error_text}"
            )));
        }

        let notification: Notification = response.json().await.map_err(|e| {
            error!("Failed to parse notification response: {}", e);
            NotificationError::InvalidResponse(e.to_string())
        })?;

        debug!("Updated notification {}", notification_id);
        Ok(notification)
    }

    /// Get a notification by ID
    #[allow(dead_code)]
    pub async fn get_notification(
        &self,
        notification_id: Uuid,
    ) -> Result<Notification, NotificationError> {
        let url = format!("{}/notifications/{}", self.base_url, notification_id);

        debug!("Getting notification {}: {}", notification_id, url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("Failed to get notification: {}", e);
            NotificationError::HttpError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NotificationError::ServiceUnavailable(format!(
                "HTTP {status}: {error_text}"
            )));
        }

        let notification: Notification = response.json().await.map_err(|e| {
            error!("Failed to parse notification response: {}", e);
            NotificationError::InvalidResponse(e.to_string())
        })?;

        Ok(notification)
    }

    /// Checks if the notification service is healthy and responding.
    ///
    /// Sends a GET request to `/health` to verify the service is reachable and functioning.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if service responds with success status, `Ok(false)` if service
    /// responds but with error status, or an error if the request fails.
    ///
    /// # Errors
    ///
    /// Returns [`NotificationError::HttpError`] if the network request fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ask::notification_client::NotificationClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NotificationClient::new("http://localhost:17004".to_string());
    ///
    /// match client.health_check().await {
    ///     Ok(true) => println!("Service is healthy"),
    ///     Ok(false) => println!("Service is running but unhealthy"),
    ///     Err(e) => eprintln!("Health check failed: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<bool, NotificationError> {
        let url = format!("{}/health", self.base_url);

        debug!("Checking notification service health: {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("Health check failed: {}", e);
            NotificationError::HttpError(e)
        })?;

        Ok(response.status().is_success())
    }

    /// Creates a notification asking if the user wants to start a tmux session.
    ///
    /// This is a convenience method that creates a pre-configured notification
    /// for the common case of asking about starting a tmux session. The notification
    /// is ephemeral (expires after 5 minutes) and requires a response.
    ///
    /// # Arguments
    ///
    /// - `request_id` - The UUID identifying this question (used as notification source)
    ///
    /// # Returns
    ///
    /// Returns `Ok(Notification)` with the created notification on success.
    ///
    /// # Errors
    ///
    /// Returns an error if notification creation fails (see [`create_notification`](Self::create_notification)).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ask::notification_client::NotificationClient;
    /// use uuid::Uuid;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NotificationClient::new("http://localhost:17004".to_string());
    /// let question_id = Uuid::new_v4();
    ///
    /// let notification = client.create_tmux_session_question(question_id).await?;
    /// println!("Created question notification: {}", notification.id);
    /// println!("Question: {}", notification.message);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_tmux_session_question(
        &self,
        request_id: Uuid,
    ) -> Result<Notification, NotificationError> {
        let request = CreateNotificationRequest {
            source: NotificationSource::AskService { request_id },
            lifetime: NotificationLifetime::ephemeral(chrono::Duration::minutes(5)),
            priority: NotificationPriority::Normal,
            title: "Start tmux session?".to_string(),
            message: "No tmux sessions are currently running. Would you like to start one?"
                .to_string(),
            requires_response: true,
        };

        self.create_notification(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_client_new() {
        let client = NotificationClient::new("http://localhost:17004".to_string());
        assert_eq!(client.base_url, "http://localhost:17004");
    }

    // Integration tests with mockito
    use crate::types::NotificationStatus;
    use mockito::Server;

    #[tokio::test]
    async fn test_create_notification_success() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
                "lifetime": {"type": "persistent"},
                "priority": "normal",
                "status": "pending",
                "title": "Test",
                "message": "Test message",
                "requires_response": true,
                "response": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }"#,
            )
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::AskService { request_id: Uuid::new_v4() },
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: true,
        };

        let result = client.create_notification(request).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert_eq!(notification.title, "Test");
        assert_eq!(notification.status, NotificationStatus::Pending);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_notification_service_error() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::AskService { request_id: Uuid::new_v4() },
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: true,
        };

        let result = client.create_notification(request).await;
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(matches!(e, NotificationError::ServiceUnavailable(_)));
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_notification_success() {
        let mut server = Server::new_async().await;
        let notification_id = Uuid::new_v4();

        let mock = server
            .mock("PUT", format!("/notifications/{notification_id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{
                "id": "{notification_id}",
                "source": {{"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"}},
                "lifetime": {{"type": "persistent"}},
                "priority": "normal",
                "status": "responded",
                "title": "Test",
                "message": "Test message",
                "requires_response": true,
                "response": "yes",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:01Z"
            }}"#))
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request = UpdateNotificationRequest {
            status: Some(NotificationStatus::Responded),
            response: Some("yes".to_string()),
        };

        let result = client.update_notification(notification_id, request).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert_eq!(notification.status, NotificationStatus::Responded);
        assert_eq!(notification.response, Some("yes".to_string()));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_notification_not_found() {
        let mut server = Server::new_async().await;
        let notification_id = Uuid::new_v4();

        let mock = server
            .mock("PUT", format!("/notifications/{notification_id}").as_str())
            .with_status(404)
            .with_body("Not Found")
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request =
            UpdateNotificationRequest { status: Some(NotificationStatus::Viewed), response: None };

        let result = client.update_notification(notification_id, request).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_notification_success() {
        let mut server = Server::new_async().await;
        let notification_id = Uuid::new_v4();

        let mock = server
            .mock("GET", format!("/notifications/{notification_id}").as_str())
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{
                "id": "{notification_id}",
                "source": {{"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"}},
                "lifetime": {{"type": "persistent"}},
                "priority": "normal",
                "status": "pending",
                "title": "Test",
                "message": "Test message",
                "requires_response": true,
                "response": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }}"#))
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let result = client.get_notification(notification_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert_eq!(notification.id, notification_id);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let mut server = Server::new_async().await;

        let mock = server.mock("GET", "/health").with_status(200).create_async().await;

        let client = NotificationClient::new(server.url());
        let result = client.health_check().await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_health_check_service_down() {
        let mut server = Server::new_async().await;

        let mock = server.mock("GET", "/health").with_status(503).create_async().await;

        let client = NotificationClient::new(server.url());
        let result = client.health_check().await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_tmux_session_question() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "source": {"type": "ask_service", "request_id": "550e8400-e29b-41d4-a716-446655440001"},
                "lifetime": {"type": "ephemeral", "expires_at": "2024-01-01T00:05:00Z"},
                "priority": "normal",
                "status": "pending",
                "title": "Start tmux session?",
                "message": "No tmux sessions are currently running. Would you like to start one?",
                "requires_response": true,
                "response": null,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }"#,
            )
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request_id = Uuid::new_v4();

        let result = client.create_tmux_session_question(request_id).await;
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert_eq!(notification.title, "Start tmux session?");
        assert!(notification.requires_response);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_response_body() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/notifications")
            .with_status(201)
            .with_body("not valid json")
            .create_async()
            .await;

        let client = NotificationClient::new(server.url());
        let request = CreateNotificationRequest {
            source: NotificationSource::AskService { request_id: Uuid::new_v4() },
            lifetime: NotificationLifetime::Persistent,
            priority: NotificationPriority::Normal,
            title: "Test".to_string(),
            message: "Test message".to_string(),
            requires_response: false,
        };

        let result = client.create_notification(request).await;
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(matches!(e, NotificationError::InvalidResponse(_)));
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_client_base_url() {
        let client = NotificationClient::new("http://example.com:8080".to_string());
        assert_eq!(client.base_url, "http://example.com:8080");
    }
}
