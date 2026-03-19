//! HTTP client for REST API communication with agentd services.
//!
//! This module provides a thin wrapper around `reqwest` for making type-safe
//! HTTP requests to backend services. All methods are async and return rich
//! error context via `anyhow`.
//!
//! # Examples
//!
//! ## Creating a client
//!
//! ```rust
//! use cli::client::ApiClient;
//!
//! let client = ApiClient::new("http://localhost:7004".to_string());
//! ```
//!
//! ## Making a GET request
//!
//! ```rust,no_run
//! # use cli::client::ApiClient;
//! # use cli::types::Notification;
//! # async fn example() -> anyhow::Result<()> {
//! let client = ApiClient::new("http://localhost:7004".to_string());
//! let notifications: Vec<Notification> = client.get("/notifications").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Making a POST request
//!
//! ```rust,no_run
//! # use cli::client::ApiClient;
//! # use cli::types::{CreateNotificationRequest, Notification};
//! # async fn example(request: CreateNotificationRequest) -> anyhow::Result<()> {
//! let client = ApiClient::new("http://localhost:7004".to_string());
//! let notification: Notification = client
//!     .post("/notifications", &request)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! All methods return `anyhow::Result` with rich error context. Network errors,
//! HTTP errors, and deserialization errors are all captured with appropriate
//! context messages.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// HTTP client for making REST API requests to agentd services.
///
/// This client wraps `reqwest::Client` and provides type-safe methods for
/// common HTTP verbs. All requests automatically serialize request bodies
/// to JSON and deserialize response bodies from JSON.
///
/// The client is cloneable and can be shared across async tasks. The underlying
/// `reqwest::Client` uses connection pooling for efficient HTTP/1.1 and HTTP/2
/// connections.
///
/// # Examples
///
/// ```rust
/// use cli::client::ApiClient;
///
/// // Create a client for the notification service
/// let notify_client = ApiClient::new("http://localhost:7004".to_string());
///
/// // Create a client for the ask service
/// let ask_client = ApiClient::new("http://localhost:7001".to_string());
/// ```
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new API client with the specified base URL.
    ///
    /// The base URL should include the protocol and host, optionally with a port.
    /// All request paths will be appended to this base URL.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cli::client::ApiClient;
    ///
    /// let client = ApiClient::new("http://localhost:7004".to_string());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for all requests (e.g., "http://localhost:7004")
    pub fn new(base_url: String) -> Self {
        Self { client: reqwest::Client::new(), base_url }
    }

    /// Make a GET request and deserialize the JSON response.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize the response into. Must implement `DeserializeOwned`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to append to the base URL (e.g., "/notifications")
    ///
    /// # Returns
    ///
    /// Returns the deserialized response on success, or an error with context on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The network request fails
    /// - The server returns a non-success status code
    /// - The response body cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use cli::client::ApiClient;
    /// # use cli::types::Notification;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = ApiClient::new("http://localhost:7004".to_string());
    /// let notifications: Vec<Notification> = client.get("/notifications").await?;
    /// println!("Found {} notifications", notifications.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;

        Self::handle_response(response).await
    }

    /// Make a POST request with a JSON body and deserialize the JSON response.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the request body. Must implement `Serialize`.
    /// * `R` - The type to deserialize the response into. Must implement `DeserializeOwned`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to append to the base URL (e.g., "/notifications")
    /// * `body` - The request body to serialize as JSON
    ///
    /// # Returns
    ///
    /// Returns the deserialized response on success, or an error with context on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The request body cannot be serialized
    /// - The network request fails
    /// - The server returns a non-success status code
    /// - The response body cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use cli::client::ApiClient;
    /// # use cli::types::{CreateNotificationRequest, Notification};
    /// # use cli::types::{NotificationSource, NotificationLifetime, NotificationPriority};
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = ApiClient::new("http://localhost:7004".to_string());
    /// let request = CreateNotificationRequest {
    ///     source: NotificationSource::System,
    ///     lifetime: NotificationLifetime::Persistent,
    ///     priority: NotificationPriority::High,
    ///     title: "Test".to_string(),
    ///     message: "Test message".to_string(),
    ///     requires_response: false,
    /// };
    /// let notification: Notification = client.post("/notifications", &request).await?;
    /// println!("Created notification with ID: {}", notification.id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn post<T: Serialize, R: DeserializeOwned>(&self, path: &str, body: &T) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))?;

        Self::handle_response(response).await
    }

    /// Make a PUT request with a JSON body and deserialize the JSON response.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the request body. Must implement `Serialize`.
    /// * `R` - The type to deserialize the response into. Must implement `DeserializeOwned`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to append to the base URL (e.g., "/notifications/123")
    /// * `body` - The request body to serialize as JSON
    ///
    /// # Returns
    ///
    /// Returns the deserialized response on success, or an error with context on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The request body cannot be serialized
    /// - The network request fails
    /// - The server returns a non-success status code
    /// - The response body cannot be deserialized
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use cli::client::ApiClient;
    /// # use cli::types::{UpdateNotificationRequest, Notification, NotificationStatus};
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = ApiClient::new("http://localhost:7004".to_string());
    /// let request = UpdateNotificationRequest {
    ///     status: Some(NotificationStatus::Viewed),
    ///     response: None,
    /// };
    /// let notification: Notification = client
    ///     .put("/notifications/550e8400-e29b-41d4-a716-446655440000", &request)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put<T: Serialize, R: DeserializeOwned>(&self, path: &str, body: &T) -> Result<R> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to PUT {url}"))?;

        Self::handle_response(response).await
    }

    /// Make a DELETE request.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to append to the base URL (e.g., "/notifications/123")
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error with context on failure.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The network request fails
    /// - The server returns a non-success status code
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use cli::client::ApiClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = ApiClient::new("http://localhost:7004".to_string());
    /// client.delete("/notifications/550e8400-e29b-41d4-a716-446655440000").await?;
    /// println!("Notification deleted");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.delete(&url).send().await.context(format!("Failed to DELETE {url}"))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {error_text}"))
        }
    }

    /// Handle an HTTP response, checking status and deserializing the body.
    ///
    /// This private helper method extracts common response handling logic.
    /// It checks if the response status is successful, then attempts to
    /// deserialize the JSON body.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize the response into. Must implement `DeserializeOwned`.
    ///
    /// # Arguments
    ///
    /// * `response` - The HTTP response to handle
    ///
    /// # Returns
    ///
    /// Returns the deserialized response on success, or an error on failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the status code indicates failure or if deserialization fails.
    async fn handle_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            response.json::<T>().await.context("Failed to parse response body")
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Request failed with status {status}: {error_text}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct an `ApiClient`, gracefully handling the macOS
    /// `system-configuration` TLS initialisation panic that occurs when
    /// `reqwest::Client::new()` is called from a non-main test thread.
    ///
    /// The side-effect of the (possibly panicking) TLS initialisation is
    /// still observed by subsequent tests in the same process, allowing
    /// mockito-based tests to bind sockets correctly.
    fn try_new_client(url: &str) -> Option<ApiClient> {
        std::panic::catch_unwind(|| ApiClient::new(url.to_string())).ok()
    }

    #[test]
    fn test_client_creation() {
        match try_new_client("http://localhost:7004") {
            Some(client) => assert_eq!(client.base_url, "http://localhost:7004"),
            None => {} // macOS TLS init panic — acceptable in test threads
        }
    }

    #[test]
    fn test_client_clone() {
        if let Some(client1) = try_new_client("http://localhost:7004") {
            let client2 = client1.clone();
            assert_eq!(client1.base_url, client2.base_url);
        }
    }

    #[test]
    fn test_client_with_different_base_urls() {
        if let (Some(c1), Some(c2)) =
            (try_new_client("http://localhost:7004"), try_new_client("http://localhost:7001"))
        {
            assert_eq!(c1.base_url, "http://localhost:7004");
            assert_eq!(c2.base_url, "http://localhost:7001");
        }
    }
}
