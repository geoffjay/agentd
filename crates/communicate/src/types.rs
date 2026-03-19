//! Domain types for the communicate service.
#![allow(dead_code)]
//!
//! Defines rooms (conversation channels), participants (agents or humans),
//! and messages, along with request/response DTOs for the REST API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// The kind of conversation a room represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    /// One-to-one conversation between exactly two participants.
    Direct,
    /// Multi-participant group conversation.
    Group,
    /// One-to-many broadcast channel (only admins post).
    Broadcast,
}

impl std::fmt::Display for RoomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomType::Direct => write!(f, "direct"),
            RoomType::Group => write!(f, "group"),
            RoomType::Broadcast => write!(f, "broadcast"),
        }
    }
}

impl std::str::FromStr for RoomType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "direct" => Ok(RoomType::Direct),
            "group" => Ok(RoomType::Group),
            "broadcast" => Ok(RoomType::Broadcast),
            _ => Err(anyhow::anyhow!("Unknown room type: {}", s)),
        }
    }
}

/// Whether a participant is an autonomous agent or a human user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantKind {
    Agent,
    Human,
}

impl std::fmt::Display for ParticipantKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParticipantKind::Agent => write!(f, "agent"),
            ParticipantKind::Human => write!(f, "human"),
        }
    }
}

impl std::str::FromStr for ParticipantKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "agent" => Ok(ParticipantKind::Agent),
            "human" => Ok(ParticipantKind::Human),
            _ => Err(anyhow::anyhow!("Unknown participant kind: {}", s)),
        }
    }
}

/// The role a participant holds within a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRole {
    /// Regular room member who can read and post.
    Member,
    /// Room administrator who can manage participants and settings.
    Admin,
    /// Read-only observer who cannot post.
    Observer,
}

impl std::fmt::Display for ParticipantRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParticipantRole::Member => write!(f, "member"),
            ParticipantRole::Admin => write!(f, "admin"),
            ParticipantRole::Observer => write!(f, "observer"),
        }
    }
}

impl std::str::FromStr for ParticipantRole {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "member" => Ok(ParticipantRole::Member),
            "admin" => Ok(ParticipantRole::Admin),
            "observer" => Ok(ParticipantRole::Observer),
            _ => Err(anyhow::anyhow!("Unknown participant role: {}", s)),
        }
    }
}

/// Delivery state of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    /// Persisted but not yet confirmed received by any participant.
    Sent,
    /// Delivered to at least one participant.
    Delivered,
    /// Read by the intended recipient(s).
    Read,
}

impl std::fmt::Display for MessageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageStatus::Sent => write!(f, "sent"),
            MessageStatus::Delivered => write!(f, "delivered"),
            MessageStatus::Read => write!(f, "read"),
        }
    }
}

impl std::str::FromStr for MessageStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sent" => Ok(MessageStatus::Sent),
            "delivered" => Ok(MessageStatus::Delivered),
            "read" => Ok(MessageStatus::Read),
            _ => Err(anyhow::anyhow!("Unknown message status: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

/// A conversation channel that groups participants and messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub room_type: RoomType,
    /// Agent UUID or human identifier of the creator.
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An agent or human who is a member of a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: Uuid,
    pub room_id: Uuid,
    /// Agent UUID or human identifier.
    pub identifier: String,
    pub kind: ParticipantKind,
    pub display_name: String,
    pub role: ParticipantRole,
    pub joined_at: DateTime<Utc>,
}

/// A message sent within a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub room_id: Uuid,
    /// Participant identifier of the sender.
    pub sender_id: String,
    /// Display name captured at send time.
    pub sender_name: String,
    pub sender_kind: ParticipantKind,
    pub content: String,
    /// Arbitrary extensible key-value metadata (e.g., echo-prevention token).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// ID of the message being replied to (threading).
    pub reply_to: Option<Uuid>,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Request body for creating a new room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    pub name: String,
    pub topic: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_room_type")]
    pub room_type: RoomType,
    pub created_by: String,
}

fn default_room_type() -> RoomType {
    RoomType::Group
}

/// Response body for room endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomResponse {
    pub id: Uuid,
    pub name: String,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub room_type: RoomType,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Room> for RoomResponse {
    fn from(r: Room) -> Self {
        Self {
            id: r.id,
            name: r.name,
            topic: r.topic,
            description: r.description,
            room_type: r.room_type,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// Request body for updating an existing room's mutable fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoomRequest {
    pub topic: Option<String>,
    pub description: Option<String>,
}

/// Request body for adding a participant to a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddParticipantRequest {
    pub identifier: String,
    pub kind: ParticipantKind,
    pub display_name: String,
    #[serde(default = "default_participant_role")]
    pub role: ParticipantRole,
}

fn default_participant_role() -> ParticipantRole {
    ParticipantRole::Member
}

/// Response body for participant endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantResponse {
    pub id: Uuid,
    pub room_id: Uuid,
    pub identifier: String,
    pub kind: ParticipantKind,
    pub display_name: String,
    pub role: ParticipantRole,
    pub joined_at: DateTime<Utc>,
}

impl From<Participant> for ParticipantResponse {
    fn from(p: Participant) -> Self {
        Self {
            id: p.id,
            room_id: p.room_id,
            identifier: p.identifier,
            kind: p.kind,
            display_name: p.display_name,
            role: p.role,
            joined_at: p.joined_at,
        }
    }
}

/// Request body for posting a message to a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub sender_id: String,
    pub sender_name: String,
    pub sender_kind: ParticipantKind,
    pub content: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    pub reply_to: Option<Uuid>,
}

/// Response body for message endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: String,
    pub sender_name: String,
    pub sender_kind: ParticipantKind,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub reply_to: Option<Uuid>,
    pub status: MessageStatus,
    pub created_at: DateTime<Utc>,
}

impl From<Message> for MessageResponse {
    fn from(m: Message) -> Self {
        Self {
            id: m.id,
            room_id: m.room_id,
            sender_id: m.sender_id,
            sender_name: m.sender_name,
            sender_kind: m.sender_kind,
            content: m.content,
            metadata: m.metadata,
            reply_to: m.reply_to,
            status: m.status,
            created_at: m.created_at,
        }
    }
}

/// Generic paginated response wrapper, matching the existing project pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_type_serde_roundtrip() {
        for variant in [RoomType::Direct, RoomType::Group, RoomType::Broadcast] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: RoomType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_participant_kind_serde_roundtrip() {
        for variant in [ParticipantKind::Agent, ParticipantKind::Human] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: ParticipantKind = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_participant_role_serde_roundtrip() {
        for variant in [ParticipantRole::Member, ParticipantRole::Admin, ParticipantRole::Observer]
        {
            let json = serde_json::to_string(&variant).unwrap();
            let back: ParticipantRole = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_message_status_serde_roundtrip() {
        for variant in [MessageStatus::Sent, MessageStatus::Delivered, MessageStatus::Read] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: MessageStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_room_type_display_fromstr() {
        let variants = [
            (RoomType::Direct, "direct"),
            (RoomType::Group, "group"),
            (RoomType::Broadcast, "broadcast"),
        ];
        for (variant, s) in &variants {
            assert_eq!(variant.to_string(), *s);
            assert_eq!(s.parse::<RoomType>().unwrap(), *variant);
        }
    }

    #[test]
    fn test_room_serde_roundtrip() {
        let room = Room {
            id: Uuid::new_v4(),
            name: "general".to_string(),
            topic: Some("Discussion".to_string()),
            description: None,
            room_type: RoomType::Group,
            created_by: "agent-abc".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&room).unwrap();
        let back: Room = serde_json::from_str(&json).unwrap();
        assert_eq!(room.id, back.id);
        assert_eq!(room.name, back.name);
        assert_eq!(room.room_type, back.room_type);
    }

    #[test]
    fn test_message_metadata_defaults() {
        let json = r#"{
            "sender_id": "agent-1",
            "sender_name": "Agent One",
            "sender_kind": "agent",
            "content": "Hello"
        }"#;
        let req: CreateMessageRequest = serde_json::from_str(json).unwrap();
        assert!(req.metadata.is_empty());
        assert!(req.reply_to.is_none());
    }

    #[test]
    fn test_create_room_request_default_type() {
        let json = r#"{
            "name": "general",
            "created_by": "human-1"
        }"#;
        let req: CreateRoomRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.room_type, RoomType::Group);
    }

    #[test]
    fn test_add_participant_request_default_role() {
        let json = r#"{
            "identifier": "agent-1",
            "kind": "agent",
            "display_name": "Agent One"
        }"#;
        let req: AddParticipantRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.role, ParticipantRole::Member);
    }

    #[test]
    fn test_paginated_response_serde() {
        let resp: PaginatedResponse<RoomResponse> =
            PaginatedResponse { items: vec![], total: 0, limit: 20, offset: 0 };
        let json = serde_json::to_string(&resp).unwrap();
        let back: PaginatedResponse<RoomResponse> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total, 0);
        assert_eq!(back.limit, 20);
    }
}
