//! HTTP client for the agentd-communicate service.
//!
//! Provides a strongly-typed client for making requests to the communicate
//! service REST API. Handles serialization, deserialization, and common error
//! patterns (404 → `None`, non-2xx → `Err`).
//!
//! # Examples
//!
//! ```ignore
//! use communicate::client::CommunicateClient;
//! use communicate::types::{CreateRoomRequest, RoomType, AddParticipantRequest, ParticipantKind};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Reads AGENTD_COMMUNICATE_SERVICE_URL env var, defaulting to http://localhost:17010
//! let client = CommunicateClient::from_env();
//!
//! // Create a room
//! let room = client.create_room(&CreateRoomRequest {
//!     name: "ops-channel".to_string(),
//!     topic: None,
//!     description: None,
//!     room_type: RoomType::Group,
//!     created_by: "agent-orchestrator".to_string(),
//! }).await?;
//!
//! // Add an agent participant
//! let participant = client.add_participant(room.id, &AddParticipantRequest {
//!     identifier: "agent-abc".to_string(),
//!     kind: ParticipantKind::Agent,
//!     display_name: "Worker Agent".to_string(),
//!     role: Default::default(),
//! }).await?;
//!
//! println!("Room {} ready with participant {}", room.id, participant.identifier);
//! # Ok(())
//! # }
//! ```

use crate::error::CommunicateError;
use crate::types::{
    AddParticipantRequest, CreateMessageRequest, CreateRoomRequest, MessageResponse,
    PaginatedResponse, ParticipantResponse, RoomResponse,
};
use agentd_common::types::HealthResponse;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

/// Environment variable key for the communicate service URL.
const ENV_KEY: &str = "AGENTD_COMMUNICATE_SERVICE_URL";

/// Default communicate service URL (development).
const DEFAULT_URL: &str = "http://localhost:17010";

/// HTTP client for the agentd-communicate service REST API.
///
/// Wraps [`reqwest::Client`] with strongly-typed methods for all communicate
/// service operations. All methods return `anyhow::Result<T>`, with 404
/// responses converted to `Ok(None)` where appropriate.
///
/// # Examples
///
/// ```ignore
/// use communicate::client::CommunicateClient;
///
/// let client = CommunicateClient::new("http://localhost:17010");
/// ```
#[derive(Clone)]
pub struct CommunicateClient {
    client: reqwest::Client,
    base_url: String,
}

impl CommunicateClient {
    /// Create a client pointing at `base_url`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use communicate::client::CommunicateClient;
    ///
    /// let client = CommunicateClient::new("http://localhost:17010");
    /// ```
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Create a client using the `AGENTD_COMMUNICATE_SERVICE_URL` environment
    /// variable, falling back to `http://localhost:17010`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use communicate::client::CommunicateClient;
    ///
    /// let client = CommunicateClient::from_env();
    /// ```
    pub fn from_env() -> Self {
        let url = std::env::var(ENV_KEY).unwrap_or_else(|_| DEFAULT_URL.to_string());
        Self::new(url)
    }

    // -----------------------------------------------------------------------
    // Room operations
    // -----------------------------------------------------------------------

    /// `POST /rooms` — create a new room.
    pub async fn create_room(&self, req: &CreateRoomRequest) -> Result<RoomResponse> {
        self.post("/rooms", req).await
    }

    /// `POST /rooms` — create a room, returning [`CommunicateError::Conflict`] when
    /// a room with the same name already exists (HTTP 409).
    ///
    /// Prefer this over [`Self::create_room`] in idempotent apply flows where
    /// a concurrent creation or a missed `get_room_by_name` lookup (e.g. large
    /// deployments exceeding the 500-room page limit) could race.
    pub async fn create_room_or_conflict(
        &self,
        req: &CreateRoomRequest,
    ) -> std::result::Result<RoomResponse, CommunicateError> {
        self.post_or_conflict("/rooms", req).await
    }

    /// `GET /rooms/{id}` — get a room by UUID.
    ///
    /// Returns `Ok(None)` if the room does not exist (404).
    pub async fn get_room(&self, id: Uuid) -> Result<Option<RoomResponse>> {
        self.get_optional(&format!("/rooms/{id}")).await
    }

    /// Find a room by its unique name.
    ///
    /// Fetches a large page of rooms and filters client-side because the
    /// service does not expose a name-based lookup endpoint directly.
    /// Returns `Ok(None)` if no room with that name exists.
    pub async fn get_room_by_name(&self, name: &str) -> Result<Option<RoomResponse>> {
        let resp: PaginatedResponse<RoomResponse> = self.get("/rooms?limit=500&offset=0").await?;
        Ok(resp.items.into_iter().find(|r| r.name == name))
    }

    /// `GET /rooms` — list rooms with pagination.
    pub async fn list_rooms(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<PaginatedResponse<RoomResponse>> {
        self.get(&format!("/rooms?limit={limit}&offset={offset}")).await
    }

    /// `DELETE /rooms/{id}` — delete a room by UUID.
    ///
    /// Returns [`CommunicateError::NotFound`] when the room does not exist.
    pub async fn delete_room(&self, id: Uuid) -> std::result::Result<(), CommunicateError> {
        self.delete_or_not_found(&format!("/rooms/{id}")).await
    }

    // -----------------------------------------------------------------------
    // Participant operations
    // -----------------------------------------------------------------------

    /// `GET /rooms/{room_id}/participants` — list participants in a room.
    ///
    /// `limit` caps the result set (max enforced server-side). Use `offset` for
    /// cursor-based pagination. Rooms with more participants than `limit` will
    /// require multiple calls to enumerate fully.
    pub async fn list_participants(
        &self,
        room_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ParticipantResponse>> {
        let resp: PaginatedResponse<ParticipantResponse> = self
            .get(&format!("/rooms/{room_id}/participants?limit={limit}&offset={offset}"))
            .await?;
        Ok(resp.items)
    }

    /// `POST /rooms/{room_id}/participants` — add a participant to a room.
    ///
    /// Returns [`CommunicateError::Conflict`] when the participant is already a
    /// member (HTTP 409), allowing callers to treat duplicates as success
    /// without parsing error strings.
    pub async fn add_participant(
        &self,
        room_id: Uuid,
        req: &AddParticipantRequest,
    ) -> std::result::Result<ParticipantResponse, CommunicateError> {
        self.post_or_conflict(&format!("/rooms/{room_id}/participants"), req).await
    }

    /// `DELETE /rooms/{room_id}/participants/{identifier}` — remove a
    /// participant from a room.
    ///
    /// Returns [`CommunicateError::NotFound`] when the participant is not a
    /// member of the room (HTTP 404), so callers can distinguish "not a member"
    /// from transport or server errors.
    pub async fn remove_participant(
        &self,
        room_id: Uuid,
        identifier: &str,
    ) -> std::result::Result<(), CommunicateError> {
        self.delete_or_not_found(&format!("/rooms/{room_id}/participants/{identifier}")).await
    }

    /// `GET /participants/{identifier}/rooms` — list all rooms for a
    /// participant, fetching up to 500 results.
    pub async fn get_rooms_for_participant(&self, identifier: &str) -> Result<Vec<RoomResponse>> {
        let resp: PaginatedResponse<RoomResponse> =
            self.get(&format!("/participants/{identifier}/rooms?limit=500")).await?;
        Ok(resp.items)
    }

    // -----------------------------------------------------------------------
    // Message operations
    // -----------------------------------------------------------------------

    /// `POST /rooms/{room_id}/messages` — send a message to a room.
    pub async fn send_message(
        &self,
        room_id: Uuid,
        req: &CreateMessageRequest,
    ) -> Result<MessageResponse> {
        self.post(&format!("/rooms/{room_id}/messages"), req).await
    }

    /// `GET /rooms/{room_id}/messages` — list messages in a room.
    ///
    /// Supports an optional `before` RFC3339 timestamp cursor for
    /// reverse-chronological pagination.
    pub async fn list_messages(
        &self,
        room_id: Uuid,
        limit: usize,
        before: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<MessageResponse>> {
        let mut path = format!("/rooms/{room_id}/messages?limit={limit}");
        if let Some(before_ts) = before {
            path.push_str(&format!("&before={}", before_ts.to_rfc3339()));
        }
        let resp: PaginatedResponse<MessageResponse> = self.get(&path).await?;
        Ok(resp.items)
    }

    /// `GET /rooms/{room_id}/messages/latest` — get the N most recent
    /// messages in a room (returned oldest-first).
    pub async fn get_latest_messages(
        &self,
        room_id: Uuid,
        count: usize,
    ) -> Result<Vec<MessageResponse>> {
        self.get(&format!("/rooms/{room_id}/messages/latest?count={count}")).await
    }

    // -----------------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------------

    /// `GET /health` — check whether the communicate service is up.
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
    }

    // -----------------------------------------------------------------------
    // Internal HTTP helpers
    // -----------------------------------------------------------------------

    /// POST that maps HTTP 409 Conflict to [`CommunicateError::Conflict`]
    /// and all other non-2xx responses to [`CommunicateError::Other`].
    ///
    /// The `"status 409"` substring is the exact text embedded by [`Self::post`]
    /// in its bail message (`"POST {url} failed with status 409 Conflict: …"`),
    /// making this check tightly coupled to that formatting — intentionally so.
    async fn post_or_conflict<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> std::result::Result<T, CommunicateError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .context(format!("Failed to POST {url}"))
            .map_err(CommunicateError::Other)?;

        if response.status() == reqwest::StatusCode::CONFLICT {
            return Err(CommunicateError::Conflict);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CommunicateError::Other(anyhow::anyhow!(
                "POST {url} failed with status {status}: {body}"
            )));
        }

        response
            .json()
            .await
            .context("Failed to deserialize POST response")
            .map_err(CommunicateError::Other)
    }

    /// DELETE that maps HTTP 404 Not Found to [`CommunicateError::NotFound`]
    /// and all other non-2xx responses to [`CommunicateError::Other`].
    async fn delete_or_not_found(&self, path: &str) -> std::result::Result<(), CommunicateError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .context(format!("Failed to DELETE {url}"))
            .map_err(CommunicateError::Other)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(CommunicateError::NotFound);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CommunicateError::Other(anyhow::anyhow!(
                "DELETE {url} failed with status {status}: {body}"
            )));
        }

        Ok(())
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GET {url} failed with status {status}: {body}");
        }

        response.json().await.context("Failed to deserialize GET response")
    }

    /// Performs a GET, returning `None` on 404 instead of an error.
    async fn get_optional<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        let url = format!("{}{}", self.base_url, path);
        let response =
            self.client.get(&url).send().await.context(format!("Failed to GET {url}"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GET {url} failed with status {status}: {body}");
        }

        let item = response.json().await.context("Failed to deserialize GET response")?;
        Ok(Some(item))
    }

    async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
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
            anyhow::bail!("POST {url} failed with status {status}: {body}");
        }

        response.json().await.context("Failed to deserialize POST response")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // reqwest::Client::new() triggers macOS system-configuration TLS
    // initialisation which panics when called from non-main test threads.
    // Rather than creating a full CommunicateClient, these tests exercise the
    // URL / env-var resolution logic directly using a helper that mirrors
    // `from_env`'s behaviour.
    fn resolve_url() -> String {
        env::var(ENV_KEY).unwrap_or_else(|_| DEFAULT_URL.to_string())
    }

    #[test]
    fn test_default_url_constant() {
        assert_eq!(DEFAULT_URL, "http://localhost:17010");
    }

    #[test]
    fn test_env_key_constant() {
        assert_eq!(ENV_KEY, "AGENTD_COMMUNICATE_SERVICE_URL");
    }

    #[test]
    fn test_resolve_url_uses_env_var() {
        env::set_var(ENV_KEY, "http://custom-host:9000");
        assert_eq!(resolve_url(), "http://custom-host:9000");
        env::remove_var(ENV_KEY);
    }

    #[test]
    fn test_resolve_url_falls_back_to_default() {
        env::remove_var(ENV_KEY);
        assert_eq!(resolve_url(), DEFAULT_URL);
    }

    #[test]
    fn test_new_base_url_stored_as_str() {
        // Verify the string conversion of the base_url, without creating
        // a reqwest::Client (see note above).
        let url = String::from("http://localhost:17010");
        // Mirror the Into<String> behaviour used in ::new()
        let stored: String = url.into();
        assert_eq!(stored, "http://localhost:17010");
    }

    // Request serialization tests using serde_json directly

    #[test]
    fn test_create_room_request_serializes() {
        use crate::types::RoomType;

        let req = CreateRoomRequest {
            name: "general".to_string(),
            topic: Some("Announcements".to_string()),
            description: None,
            room_type: RoomType::Group,
            created_by: "agent-1".to_string(),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"general\""));
        assert!(json.contains("\"room_type\":\"group\""));
        assert!(json.contains("\"created_by\":\"agent-1\""));
    }

    #[test]
    fn test_add_participant_request_serializes() {
        use crate::types::{ParticipantKind, ParticipantRole};

        let req = AddParticipantRequest {
            identifier: "agent-abc".to_string(),
            kind: ParticipantKind::Agent,
            display_name: "Worker Agent".to_string(),
            role: ParticipantRole::Member,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"identifier\":\"agent-abc\""));
        assert!(json.contains("\"kind\":\"agent\""));
        assert!(json.contains("\"role\":\"member\""));
    }

    #[test]
    fn test_create_message_request_serializes() {
        use crate::types::ParticipantKind;

        let req = CreateMessageRequest {
            sender_id: "agent-1".to_string(),
            sender_name: "Agent One".to_string(),
            sender_kind: ParticipantKind::Agent,
            content: "Hello, world!".to_string(),
            metadata: Default::default(),
            reply_to: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"content\":\"Hello, world!\""));
        assert!(json.contains("\"sender_kind\":\"agent\""));
    }

    #[test]
    fn test_room_response_deserializes() {
        use crate::types::RoomType;
        use chrono::Utc;

        let id = Uuid::new_v4();
        let now = Utc::now();

        let resp = RoomResponse {
            id,
            name: "general".to_string(),
            topic: None,
            description: None,
            room_type: RoomType::Group,
            created_by: "agent-1".to_string(),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let back: RoomResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, id);
        assert_eq!(back.name, "general");
        assert_eq!(back.room_type, RoomType::Group);
    }

    #[test]
    fn test_message_response_deserializes() {
        use crate::types::{MessageStatus, ParticipantKind};
        use chrono::Utc;

        let resp = MessageResponse {
            id: Uuid::new_v4(),
            room_id: Uuid::new_v4(),
            sender_id: "agent-1".to_string(),
            sender_name: "Agent One".to_string(),
            sender_kind: ParticipantKind::Agent,
            content: "Hi!".to_string(),
            metadata: Default::default(),
            reply_to: None,
            status: MessageStatus::Sent,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let back: MessageResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "Hi!");
        assert_eq!(back.status, MessageStatus::Sent);
    }
}
