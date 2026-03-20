//! Communicate service command implementations.
//!
//! Provides CLI subcommands for managing rooms, participants, and messages via
//! the agentd-communicate service. The service runs on port 17010 by default.
//!
//! # Available Commands
//!
//! ## Room management
//! - **create-room**: Create a new conversation room
//! - **list-rooms**: List all rooms with pagination
//! - **get-room**: Get details of a specific room by UUID or name
//! - **delete-room**: Delete a room by UUID or name
//!
//! ## Participant management
//! - **join**: Add a participant (agent or human) to a room
//! - **leave**: Remove a participant from a room
//! - **members**: List all participants in a room
//!
//! ## Messaging
//! - **send**: Send a message to a room as a participant
//! - **messages**: Fetch recent messages from a room
//! - **watch**: Live-tail room messages via WebSocket
//!
//! # Examples
//!
//! ```bash
//! # Create a group room
//! agent communicate create-room --name ops-channel --room-type group
//!
//! # List rooms
//! agent communicate list-rooms --limit 20
//!
//! # Send a message
//! agent communicate send ops-channel --from my-agent --message "Deploy complete"
//!
//! # Live-tail a room
//! agent communicate watch ops-channel
//! ```

use anyhow::{Context, Result};
use chrono::DateTime;
use clap::Subcommand;
use colored::*;
use communicate::client::CommunicateClient;
use communicate::error::CommunicateError;
use communicate::types::{
    AddParticipantRequest, CreateMessageRequest, CreateRoomRequest, MessageResponse,
    ParticipantKind, ParticipantResponse, ParticipantRole, RoomResponse, RoomType,
};
use futures_util::{SinkExt, StreamExt};
use prettytable::{format, Cell, Row, Table};
use std::collections::HashMap;
use uuid::Uuid;

/// Communicate service subcommands.
#[derive(Debug, Subcommand)]
pub enum CommunicateCommand {
    /// Check the health of the communicate service.
    Health,

    // -----------------------------------------------------------------------
    // Room management
    // -----------------------------------------------------------------------
    /// Create a new conversation room.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate create-room --name ops-channel
    /// agent communicate create-room --name alerts --room-type broadcast --topic "System alerts"
    /// ```
    CreateRoom {
        /// Room name (must be unique)
        #[arg(long)]
        name: String,

        /// Optional topic / short description
        #[arg(long)]
        topic: Option<String>,

        /// Optional long description
        #[arg(long)]
        description: Option<String>,

        /// Room type: direct, group (default), or broadcast
        #[arg(long, default_value = "group")]
        room_type: String,

        /// Identifier recorded as the room creator for attribution and audit purposes.
        /// Cannot be changed after creation.
        #[arg(long, default_value = "cli")]
        created_by: String,
    },

    /// List rooms with optional pagination.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate list-rooms
    /// agent communicate list-rooms --limit 10 --offset 20
    /// agent communicate list-rooms --json
    /// ```
    ListRooms {
        /// Maximum number of rooms to return (default: 20)
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Offset for pagination (default: 0)
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Get details of a specific room by UUID or name.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate get-room ops-channel
    /// agent communicate get-room 550e8400-e29b-41d4-a716-446655440000
    /// ```
    GetRoom {
        /// Room UUID or name
        room: String,
    },

    /// Delete a room by UUID or name.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate delete-room ops-channel
    /// agent communicate delete-room 550e8400-e29b-41d4-a716-446655440000
    /// ```
    DeleteRoom {
        /// Room UUID or name
        room: String,
    },

    // -----------------------------------------------------------------------
    // Participant management
    // -----------------------------------------------------------------------
    /// Add a participant (agent or human) to a room.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate join ops-channel --identifier my-agent --kind agent
    /// agent communicate join general --identifier alice --kind human --display-name "Alice"
    /// ```
    Join {
        /// Room UUID or name
        room: String,

        /// Participant identifier (agent UUID or human username)
        #[arg(long)]
        identifier: String,

        /// Participant kind: agent or human
        #[arg(long, default_value = "agent")]
        kind: String,

        /// Display name shown in participant lists
        #[arg(long)]
        display_name: Option<String>,

        /// Role in the room: member (default), admin, or observer
        #[arg(long, default_value = "member")]
        role: String,
    },

    /// Remove a participant from a room.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate leave ops-channel --identifier my-agent
    /// ```
    Leave {
        /// Room UUID or name
        room: String,

        /// Participant identifier to remove
        #[arg(long)]
        identifier: String,
    },

    /// List all participants in a room.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate members ops-channel
    /// agent communicate members ops-channel --limit 50 --offset 0
    /// agent communicate members ops-channel --json
    /// ```
    Members {
        /// Room UUID or name
        room: String,

        /// Maximum number of participants to return (default: 100)
        #[arg(long, default_value = "100")]
        limit: usize,

        /// Offset for pagination (default: 0)
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    // -----------------------------------------------------------------------
    // Messaging
    // -----------------------------------------------------------------------
    /// Send a message to a room as a participant.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate send ops-channel --from my-agent --message "Deploy complete"
    /// agent communicate send alerts --from monitor --message "CPU high" \
    ///   --metadata severity=high --metadata host=web-01
    /// ```
    Send {
        /// Room UUID or name
        room: String,

        /// Sender identifier (agent UUID or human username)
        #[arg(long)]
        from: String,

        /// Message content
        #[arg(long)]
        message: String,

        /// Optional metadata as key=value pairs (repeatable)
        #[arg(long, value_name = "KEY=VALUE")]
        metadata: Vec<String>,

        /// Sender kind: agent (default) or human
        #[arg(long, default_value = "agent")]
        kind: String,

        /// Sender display name (defaults to --from value)
        #[arg(long)]
        display_name: Option<String>,
    },

    /// Fetch recent messages from a room.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate messages ops-channel
    /// agent communicate messages ops-channel --limit 50
    /// agent communicate messages ops-channel --before 2025-01-01T00:00:00Z
    /// ```
    Messages {
        /// Room UUID or name
        room: String,

        /// Maximum number of messages to return (default: 20)
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Return only messages before this RFC3339 timestamp
        #[arg(long)]
        before: Option<String>,
    },

    /// Live-tail room messages via WebSocket.
    ///
    /// Connects to the communicate service WebSocket, subscribes to the room,
    /// and streams new messages to stdout until Ctrl+C is pressed.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent communicate watch ops-channel
    /// agent communicate watch ops-channel --identifier alice --kind human
    /// agent communicate watch ops-channel --json
    /// ```
    Watch {
        /// Room UUID or name
        room: String,

        /// Identifier used to connect to the WebSocket (default: "cli-observer")
        #[arg(long, default_value = "cli-observer")]
        identifier: String,

        /// Kind: agent or human (default: human)
        #[arg(long, default_value = "human")]
        kind: String,

        /// Display name for this watcher (default: "CLI")
        #[arg(long, default_value = "CLI")]
        display_name: String,
    },
}

impl CommunicateCommand {
    /// Execute the communicate command.
    pub async fn execute(
        &self,
        client: &CommunicateClient,
        base_url: &str,
        json: bool,
    ) -> Result<()> {
        match self {
            CommunicateCommand::Health => health(client, json).await,
            CommunicateCommand::CreateRoom { name, topic, description, room_type, created_by } => {
                create_room(
                    client,
                    name,
                    topic.as_deref(),
                    description.as_deref(),
                    room_type,
                    created_by,
                    json,
                )
                .await
            }
            CommunicateCommand::ListRooms { limit, offset } => {
                list_rooms(client, *limit, *offset, json).await
            }
            CommunicateCommand::GetRoom { room } => get_room(client, room, json).await,
            CommunicateCommand::DeleteRoom { room } => delete_room(client, room, json).await,
            CommunicateCommand::Join { room, identifier, kind, display_name, role } => {
                join_room(client, room, identifier, kind, display_name.as_deref(), role, json).await
            }
            CommunicateCommand::Leave { room, identifier } => {
                leave_room(client, room, identifier, json).await
            }
            CommunicateCommand::Members { room, limit, offset } => {
                list_members(client, room, *limit, *offset, json).await
            }
            CommunicateCommand::Send { room, from, message, metadata, kind, display_name } => {
                send_message(
                    client,
                    room,
                    from,
                    message,
                    metadata,
                    kind,
                    display_name.as_deref(),
                    json,
                )
                .await
            }
            CommunicateCommand::Messages { room, limit, before } => {
                list_messages(client, room, *limit, before.as_deref(), json).await
            }
            CommunicateCommand::Watch { room, identifier, kind, display_name } => {
                watch_room(client, base_url, room, identifier, kind, display_name, json).await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: resolve a room argument (UUID or name) to a RoomResponse
// ---------------------------------------------------------------------------

async fn resolve_room(client: &CommunicateClient, room: &str) -> Result<RoomResponse> {
    // Try parsing as a UUID first; fall back to name lookup.
    let room_resp = if let Ok(id) = Uuid::parse_str(room) {
        client
            .get_room(id)
            .await
            .context("Failed to reach communicate service")?
            .ok_or_else(|| anyhow::anyhow!("Room not found: {}", room))?
    } else {
        client
            .get_room_by_name(room)
            .await
            .context("Failed to reach communicate service")?
            .ok_or_else(|| anyhow::anyhow!("Room not found: {}", room))?
    };
    Ok(room_resp)
}

/// Parse `key=value` metadata strings into a `HashMap`.
fn parse_metadata(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in pairs {
        let (k, v) = pair.split_once('=').ok_or_else(|| {
            anyhow::anyhow!("Invalid metadata format '{pair}': expected key=value")
        })?;
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health(client: &CommunicateClient, json: bool) -> Result<()> {
    client.health().await.context("Failed to reach communicate service. Is it running?")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status": "ok"}))?);
    } else {
        println!("{} {}", "communicate:".bold(), "ok".green().bold());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_room(
    client: &CommunicateClient,
    name: &str,
    topic: Option<&str>,
    description: Option<&str>,
    room_type_str: &str,
    created_by: &str,
    json: bool,
) -> Result<()> {
    let room_type = parse_room_type(room_type_str)?;
    let room = client
        .create_room(&CreateRoomRequest {
            name: name.to_string(),
            topic: topic.map(str::to_string),
            description: description.map(str::to_string),
            room_type,
            created_by: created_by.to_string(),
        })
        .await
        .context("Failed to create room")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&room)?);
    } else {
        println!("{}", "Room created successfully!".green().bold());
        println!();
        display_room(&room);
    }
    Ok(())
}

async fn list_rooms(
    client: &CommunicateClient,
    limit: usize,
    offset: usize,
    json: bool,
) -> Result<()> {
    let resp = client.list_rooms(limit, offset).await.context("Failed to list rooms")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
        return Ok(());
    }

    if resp.items.is_empty() {
        println!("{}", "No rooms found.".yellow());
        return Ok(());
    }

    println!("{}", format!("Showing {}/{} room(s)", resp.items.len(), resp.total).cyan().bold());
    println!();

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);
    table.set_titles(Row::new(vec![
        Cell::new("ID").style_spec("Fb"),
        Cell::new("Name").style_spec("Fb"),
        Cell::new("Type").style_spec("Fb"),
        Cell::new("Topic").style_spec("Fb"),
        Cell::new("Created By").style_spec("Fb"),
        Cell::new("Created At").style_spec("Fb"),
    ]));

    for room in &resp.items {
        table.add_row(Row::new(vec![
            Cell::new(&room.id.to_string()),
            Cell::new(&room.name),
            Cell::new(&room.room_type.to_string()),
            Cell::new(room.topic.as_deref().unwrap_or("-")),
            Cell::new(&room.created_by),
            Cell::new(&room.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    table.printstd();
    Ok(())
}

async fn get_room(client: &CommunicateClient, room: &str, json: bool) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&room_resp)?);
    } else {
        display_room(&room_resp);
    }
    Ok(())
}

async fn delete_room(client: &CommunicateClient, room: &str, json: bool) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;

    match client.delete_room(room_resp.id).await {
        Ok(()) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "success": true,
                        "id": room_resp.id,
                        "name": room_resp.name,
                    }))?
                );
            } else {
                println!(
                    "{}",
                    format!("Room '{}' ({}) deleted successfully.", room_resp.name, room_resp.id)
                        .green()
                        .bold()
                );
            }
        }
        Err(CommunicateError::NotFound) => {
            anyhow::bail!("Room not found: {}", room);
        }
        Err(e) => {
            return Err(anyhow::anyhow!(e).context("Failed to delete room"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn join_room(
    client: &CommunicateClient,
    room: &str,
    identifier: &str,
    kind_str: &str,
    display_name: Option<&str>,
    role_str: &str,
    json: bool,
) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;
    let kind = parse_participant_kind(kind_str)?;
    let role = parse_participant_role(role_str)?;
    let display_name = display_name.unwrap_or(identifier).to_string();

    let participant = match client
        .add_participant(
            room_resp.id,
            &AddParticipantRequest { identifier: identifier.to_string(), kind, display_name, role },
        )
        .await
    {
        Ok(p) => p,
        Err(CommunicateError::Conflict) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "status": "already_member",
                        "room_id": room_resp.id,
                        "identifier": identifier,
                    }))?
                );
            } else {
                println!(
                    "{}",
                    format!("'{}' is already a member of '{}'.", identifier, room_resp.name)
                        .yellow()
                );
            }
            return Ok(());
        }
        Err(e) => return Err(anyhow::anyhow!(e).context("Failed to join room")),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&participant)?);
    } else {
        println!(
            "{}",
            format!("'{}' joined room '{}' successfully.", identifier, room_resp.name)
                .green()
                .bold()
        );
        println!();
        display_participant(&participant);
    }
    Ok(())
}

async fn leave_room(
    client: &CommunicateClient,
    room: &str,
    identifier: &str,
    json: bool,
) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;

    match client.remove_participant(room_resp.id, identifier).await {
        Ok(()) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "success": true,
                        "room_id": room_resp.id,
                        "identifier": identifier,
                    }))?
                );
            } else {
                println!(
                    "{}",
                    format!("'{}' removed from room '{}'.", identifier, room_resp.name)
                        .green()
                        .bold()
                );
            }
        }
        Err(CommunicateError::NotFound) => {
            anyhow::bail!("'{}' is not a member of room '{}'.", identifier, room_resp.name);
        }
        Err(e) => return Err(anyhow::anyhow!(e).context("Failed to leave room")),
    }
    Ok(())
}

async fn list_members(
    client: &CommunicateClient,
    room: &str,
    limit: usize,
    offset: usize,
    json: bool,
) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;
    let participants = client
        .list_participants(room_resp.id, limit, offset)
        .await
        .context("Failed to list participants")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&participants)?);
        return Ok(());
    }

    if participants.is_empty() {
        println!("{}", format!("No participants in '{}'.", room_resp.name).yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("{} participant(s) in '{}':", participants.len(), room_resp.name).cyan().bold()
    );
    println!();

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);
    table.set_titles(Row::new(vec![
        Cell::new("Identifier").style_spec("Fb"),
        Cell::new("Kind").style_spec("Fb"),
        Cell::new("Display Name").style_spec("Fb"),
        Cell::new("Role").style_spec("Fb"),
        Cell::new("Joined At").style_spec("Fb"),
    ]));

    for p in &participants {
        let kind_style = match p.kind {
            ParticipantKind::Agent => "Fc",
            ParticipantKind::Human => "Fg",
        };
        let role_style = match p.role {
            ParticipantRole::Admin => "Fy",
            ParticipantRole::Observer => "Fd",
            ParticipantRole::Member => "",
        };
        table.add_row(Row::new(vec![
            Cell::new(&p.identifier),
            Cell::new(&p.kind.to_string()).style_spec(kind_style),
            Cell::new(&p.display_name),
            Cell::new(&p.role.to_string()).style_spec(role_style),
            Cell::new(&p.joined_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    table.printstd();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn send_message(
    client: &CommunicateClient,
    room: &str,
    from: &str,
    content: &str,
    metadata_pairs: &[String],
    kind_str: &str,
    display_name: Option<&str>,
    json: bool,
) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;
    let kind = parse_participant_kind(kind_str)?;
    let metadata = parse_metadata(metadata_pairs)?;
    let sender_name = display_name.unwrap_or(from).to_string();

    let message = client
        .send_message(
            room_resp.id,
            &CreateMessageRequest {
                sender_id: from.to_string(),
                sender_name,
                sender_kind: kind,
                content: content.to_string(),
                metadata,
                reply_to: None,
            },
        )
        .await
        .context("Failed to send message")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&message)?);
    } else {
        println!("{}", "Message sent successfully!".green().bold());
        println!();
        display_message(&message);
    }
    Ok(())
}

async fn list_messages(
    client: &CommunicateClient,
    room: &str,
    limit: usize,
    before_str: Option<&str>,
    json: bool,
) -> Result<()> {
    let room_resp = resolve_room(client, room).await?;

    let messages = if let Some(before_s) = before_str {
        let before: DateTime<chrono::Utc> =
            before_s.parse().context("Invalid --before timestamp (expected RFC3339)")?;
        client
            .list_messages(room_resp.id, limit, Some(before))
            .await
            .context("Failed to list messages")?
    } else {
        client.get_latest_messages(room_resp.id, limit).await.context("Failed to list messages")?
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&messages)?);
        return Ok(());
    }

    if messages.is_empty() {
        println!("{}", format!("No messages in '{}'.", room_resp.name).yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("{} message(s) from '{}':", messages.len(), room_resp.name).cyan().bold()
    );
    println!();

    for msg in &messages {
        display_message(msg);
        println!();
    }
    Ok(())
}

/// Build the WebSocket URL from the HTTP base URL and connection parameters.
///
/// Converts `http://` → `ws://` and `https://` → `wss://`, then appends
/// the participant query-string parameters, each percent-encoded.
fn build_ws_url(base_url: &str, identifier: &str, kind: &str, display_name: &str) -> String {
    let ws_base = base_url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let identifier_enc = urlencoding::encode(identifier);
    let kind_enc = urlencoding::encode(kind);
    let display_name_enc = urlencoding::encode(display_name);
    format!(
        "{ws_base}/ws?identifier={identifier_enc}&kind={kind_enc}&display_name={display_name_enc}"
    )
}

/// Live-tail a room's messages via the communicate service WebSocket.
///
/// Auto-joins the watcher as an observer before subscribing, and removes the
/// observer on clean disconnect if it was auto-joined (i.e. was not already
/// a participant). Sends a proper WebSocket close frame on Ctrl+C.
async fn watch_room(
    client: &CommunicateClient,
    base_url: &str,
    room: &str,
    identifier: &str,
    kind_str: &str,
    display_name: &str,
    json: bool,
) -> Result<()> {
    // Resolve the room first to get its UUID.
    let room_resp = resolve_room(client, room).await?;

    let kind = parse_participant_kind(kind_str)?;

    // Auto-join as observer if not already a member. Track whether we joined
    // so we can clean up on disconnect.
    let auto_joined = match client
        .add_participant(
            room_resp.id,
            &AddParticipantRequest {
                identifier: identifier.to_string(),
                kind,
                display_name: display_name.to_string(),
                role: ParticipantRole::Observer,
            },
        )
        .await
    {
        Ok(_) => {
            if !json {
                eprintln!("{}", format!("Joined '{}' as observer.", room_resp.name).bright_black());
            }
            true
        }
        // Already a member — don't remove them when we disconnect.
        Err(CommunicateError::Conflict) => false,
        Err(e) => return Err(anyhow::anyhow!(e).context("Failed to join room as observer")),
    };

    // Build the WebSocket URL, percent-encoding all query parameters.
    let ws_url = build_ws_url(base_url, identifier, kind_str, display_name);

    if !json {
        eprintln!(
            "{}",
            format!("Connecting to room '{}' ({})...", room_resp.name, room_resp.id).cyan()
        );
        eprintln!("{}", "Press Ctrl+C to disconnect.".bright_black());
        eprintln!();
    }

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .context("Failed to connect to communicate WebSocket. Is the service running?")?;

    let (mut write, mut read) = ws_stream.split();

    // Send subscribe message for our room.
    let subscribe_msg = serde_json::json!({
        "type": "subscribe",
        "room_id": room_resp.id,
    });
    write
        .send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string().into()))
        .await
        .context("Failed to subscribe to room")?;

    if !json {
        eprintln!("{}", format!("Watching room '{}'...", room_resp.name).green());
        eprintln!();
    }

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                        if json {
                            println!("{}", text);
                        } else {
                            format_watch_message(&text, &room_resp.name);
                        }
                    }
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                        if !json {
                            eprintln!("{}", "Connection closed by server.".yellow());
                        }
                        break;
                    }
                    Some(Err(e)) => {
                        if !json {
                            eprintln!("{}", format!("WebSocket error: {e}").red());
                        }
                        break;
                    }
                    None => {
                        if !json {
                            eprintln!("{}", "Stream ended.".yellow());
                        }
                        break;
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                // Send a clean WebSocket close frame before disconnecting.
                write.send(tokio_tungstenite::tungstenite::Message::Close(None)).await.ok();
                if !json {
                    eprintln!();
                    eprintln!("{}", "Disconnected.".yellow());
                }
                break;
            }
        }
    }

    // If we auto-joined as an observer, remove that participant so we don't
    // leave phantom observers in the room.
    if auto_joined {
        if let Err(e) = client.remove_participant(room_resp.id, identifier).await {
            if !json {
                eprintln!(
                    "{}",
                    format!("Warning: failed to remove observer on disconnect: {e}").yellow()
                );
            }
        }
    }

    Ok(())
}

/// Format and display a WebSocket server message for human consumption.
fn format_watch_message(text: &str, room_name: &str) {
    let msg: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            println!("{}", text.bright_black());
            return;
        }
    };

    match msg.get("type").and_then(|t| t.as_str()) {
        Some("message") => {
            if let Some(m) = msg.get("message") {
                let sender = m
                    .get("sender_name")
                    .or_else(|| m.get("sender_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let ts = m
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<DateTime<chrono::Utc>>().ok())
                    .map(|t| t.format("%H:%M:%S").to_string())
                    .unwrap_or_default();

                println!(
                    "{} {} {}: {}",
                    ts.bright_black(),
                    format!("[{room_name}]").cyan(),
                    sender.bold(),
                    content,
                );
            }
        }
        Some("participant_event") => {
            let event = msg.get("event").and_then(|v| v.as_str()).unwrap_or("?");
            let who = msg
                .get("participant")
                .and_then(|p| p.get("display_name").or_else(|| p.get("identifier")))
                .and_then(|v| v.as_str())
                .or_else(|| msg.get("identifier").and_then(|v| v.as_str()))
                .unwrap_or("someone");
            eprintln!(
                "{} {} {}",
                format!("[{room_name}]").cyan(),
                who.bright_black(),
                format!("{event} the room").bright_black(),
            );
        }
        Some("error") => {
            let err = msg.get("message").and_then(|v| v.as_str()).unwrap_or("unknown error");
            eprintln!("{}", format!("Server error: {err}").red());
        }
        Some("pong") => {}
        _ => {
            println!("{}", text.bright_black());
        }
    }
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn display_room(room: &RoomResponse) {
    println!("{}", "═".repeat(70).cyan());
    println!("{}: {}", "ID".bold(), room.id.to_string().bright_black());
    println!("{}: {}", "Name".bold(), room.name.bright_white().bold());
    println!("{}: {}", "Type".bold(), room.room_type.to_string().cyan());
    if let Some(ref t) = room.topic {
        println!("{}: {}", "Topic".bold(), t);
    }
    if let Some(ref d) = room.description {
        println!("{}: {}", "Description".bold(), d);
    }
    println!("{}: {}", "Created By".bold(), room.created_by);
    println!("{}: {}", "Created At".bold(), room.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("{}: {}", "Updated At".bold(), room.updated_at.format("%Y-%m-%d %H:%M:%S"));
    println!("{}", "═".repeat(70).cyan());
}

fn display_participant(p: &ParticipantResponse) {
    println!("{}", "─".repeat(50).bright_black());
    println!("{}: {}", "Identifier".bold(), p.identifier);
    println!("{}: {}", "Kind".bold(), p.kind.to_string().cyan());
    println!("{}: {}", "Display Name".bold(), p.display_name);
    println!("{}: {}", "Role".bold(), p.role);
    println!("{}: {}", "Joined At".bold(), p.joined_at.format("%Y-%m-%d %H:%M:%S"));
    println!("{}", "─".repeat(50).bright_black());
}

fn display_message(msg: &MessageResponse) {
    let ts = msg.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
    println!(
        "{} {} ({}): {}",
        ts.bright_black(),
        msg.sender_name.bold(),
        msg.sender_kind.to_string().cyan(),
        msg.content,
    );
    if !msg.metadata.is_empty() {
        let meta: Vec<String> = msg.metadata.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("  {}: {}", "metadata".bright_black(), meta.join(", ").bright_black());
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_room_type(s: &str) -> Result<RoomType> {
    match s.to_lowercase().as_str() {
        "direct" => Ok(RoomType::Direct),
        "group" => Ok(RoomType::Group),
        "broadcast" => Ok(RoomType::Broadcast),
        other => {
            anyhow::bail!("Invalid room type '{}': expected direct, group, or broadcast", other)
        }
    }
}

fn parse_participant_kind(s: &str) -> Result<ParticipantKind> {
    match s.to_lowercase().as_str() {
        "agent" => Ok(ParticipantKind::Agent),
        "human" => Ok(ParticipantKind::Human),
        other => anyhow::bail!("Invalid participant kind '{}': expected agent or human", other),
    }
}

fn parse_participant_role(s: &str) -> Result<ParticipantRole> {
    match s.to_lowercase().as_str() {
        "member" => Ok(ParticipantRole::Member),
        "admin" => Ok(ParticipantRole::Admin),
        "observer" => Ok(ParticipantRole::Observer),
        other => anyhow::bail!("Invalid role '{}': expected member, admin, or observer", other),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_room_type() {
        assert!(matches!(parse_room_type("direct").unwrap(), RoomType::Direct));
        assert!(matches!(parse_room_type("group").unwrap(), RoomType::Group));
        assert!(matches!(parse_room_type("broadcast").unwrap(), RoomType::Broadcast));
        assert!(matches!(parse_room_type("GROUP").unwrap(), RoomType::Group));
        assert!(parse_room_type("invalid").is_err());
    }

    #[test]
    fn test_parse_participant_kind() {
        assert!(matches!(parse_participant_kind("agent").unwrap(), ParticipantKind::Agent));
        assert!(matches!(parse_participant_kind("human").unwrap(), ParticipantKind::Human));
        assert!(matches!(parse_participant_kind("AGENT").unwrap(), ParticipantKind::Agent));
        assert!(parse_participant_kind("robot").is_err());
    }

    #[test]
    fn test_parse_participant_role() {
        assert!(matches!(parse_participant_role("member").unwrap(), ParticipantRole::Member));
        assert!(matches!(parse_participant_role("admin").unwrap(), ParticipantRole::Admin));
        assert!(matches!(parse_participant_role("observer").unwrap(), ParticipantRole::Observer));
        assert!(parse_participant_role("owner").is_err());
    }

    #[test]
    fn test_parse_metadata_valid() {
        let pairs = vec!["key=value".to_string(), "severity=high".to_string()];
        let map = parse_metadata(&pairs).unwrap();
        assert_eq!(map.get("key").unwrap(), "value");
        assert_eq!(map.get("severity").unwrap(), "high");
    }

    #[test]
    fn test_parse_metadata_invalid() {
        let pairs = vec!["noequals".to_string()];
        assert!(parse_metadata(&pairs).is_err());
    }

    #[test]
    fn test_parse_metadata_value_with_equals() {
        // Values that contain '=' after the first '=' should be kept intact
        let pairs = vec!["token=abc=def".to_string()];
        let map = parse_metadata(&pairs).unwrap();
        assert_eq!(map.get("token").unwrap(), "abc=def");
    }

    // -----------------------------------------------------------------------
    // WebSocket URL construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_ws_url_http_to_ws() {
        let url = build_ws_url("http://localhost:17010", "cli-observer", "human", "CLI");
        assert!(url.starts_with("ws://"), "expected ws:// scheme, got: {url}");
        assert!(url.contains("/ws?"), "expected /ws? path: {url}");
        assert!(url.contains("identifier=cli-observer"));
        assert!(url.contains("kind=human"));
        assert!(url.contains("display_name=CLI"));
    }

    #[test]
    fn test_build_ws_url_https_to_wss() {
        let url = build_ws_url("https://example.com", "user", "agent", "My Agent");
        assert!(url.starts_with("wss://"), "expected wss:// scheme, got: {url}");
    }

    #[test]
    fn test_build_ws_url_encodes_identifier_with_special_chars() {
        let url = build_ws_url("http://localhost:17010", "user@host.com", "human", "Display Name");
        // '@' should be percent-encoded
        assert!(url.contains("user%40host.com"), "expected encoded @: {url}");
        // spaces in display_name should be encoded
        assert!(url.contains("Display%20Name"), "expected encoded space: {url}");
    }

    #[test]
    fn test_build_ws_url_encodes_kind() {
        // kind with special chars (unlikely but should be safe)
        let url = build_ws_url("http://localhost:17010", "id", "agent type", "Name");
        assert!(url.contains("agent%20type"), "expected encoded space in kind: {url}");
    }

    // -----------------------------------------------------------------------
    // format_watch_message smoke tests (ensures no panics on each type)
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_watch_message_pong_no_panic() {
        format_watch_message(r#"{"type":"pong"}"#, "test-room");
    }

    #[test]
    fn test_format_watch_message_error_no_panic() {
        format_watch_message(
            r#"{"type":"error","message":"you are not a participant"}"#,
            "test-room",
        );
    }

    #[test]
    fn test_format_watch_message_participant_event_no_panic() {
        format_watch_message(
            r#"{"type":"participant_event","event":"joined","participant":{"display_name":"Alice","identifier":"alice"}}"#,
            "test-room",
        );
    }

    #[test]
    fn test_format_watch_message_message_no_panic() {
        format_watch_message(
            r#"{"type":"message","message":{"sender_name":"bot","sender_id":"bot-1","content":"hello","created_at":"2026-01-01T00:00:00Z"}}"#,
            "test-room",
        );
    }

    #[test]
    fn test_format_watch_message_unknown_type_no_panic() {
        format_watch_message(r#"{"type":"unknown_future_type","data":{}}"#, "test-room");
    }

    #[test]
    fn test_format_watch_message_invalid_json_no_panic() {
        format_watch_message("this is not json", "test-room");
    }
}
