//! Room, participant, and message inspection tools for the agentd communicate
//! service.
//!
//! These tools give MCP clients visibility into agent-to-agent and
//! human-to-agent communication channels, and allow sending messages into a
//! room for remediation flows (e.g. the system agent posting a status update).

use crate::client::AgentdClient;
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Room {
    id: String,
    name: String,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    description: Option<String>,
    room_type: String,
    created_by: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct Message {
    id: String,
    sender_id: String,
    sender_name: String,
    sender_kind: String,
    content: String,
    status: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct Participant {
    identifier: String,
    kind: String,
    display_name: String,
    role: String,
    joined_at: String,
}

#[derive(Debug, Deserialize)]
struct Paginated<T> {
    items: Vec<T>,
    total: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn room_type_icon(rt: &str) -> &'static str {
    match rt {
        "direct" => "👤",
        "group" => "👥",
        "broadcast" => "📢",
        _ => "💬",
    }
}

fn sender_icon(kind: &str) -> &'static str {
    match kind {
        "agent" => "🤖",
        "human" => "🧑",
        _ => "❓",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', " ")
    } else {
        let mut t = s[..max].replace('\n', " ");
        t.push('…');
        t
    }
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

pub async fn run_list_rooms(
    client: &AgentdClient,
    room_type: Option<&str>,
    project_id: Option<&str>,
    limit: Option<u32>,
) -> String {
    let base = client.communicate_url();
    let limit_val = limit.unwrap_or(50).clamp(1, 200);
    let mut url = format!("{base}/rooms?limit={limit_val}");
    if let Some(rt) = room_type {
        url.push_str(&format!("&room_type={rt}"));
    }
    if let Some(pid) = project_id {
        url.push_str(&format!("&project_id={pid}"));
    }

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "Error: communicate service unreachable at {base}. Is it running?\nDetails: {e}"
            );
        }
    };
    if !resp.status().is_success() {
        return format!("Error: communicate returned HTTP {}", resp.status());
    }
    let page: Paginated<Room> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing rooms: {e}"),
    };

    if page.items.is_empty() {
        return "No rooms found.".to_string();
    }

    let mut out = format!("## Rooms — {} total\n\n", page.total);
    out.push_str("| Type | Name | ID | Topic | Created |\n");
    out.push_str("|------|------|-----|-------|---------|\n");
    for r in &page.items {
        let icon = room_type_icon(&r.room_type);
        let topic = r.topic.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "| {icon} {} | {} | `{}` | {} | {} |\n",
            r.room_type,
            r.name,
            r.id,
            truncate(topic, 40),
            r.created_at
        ));
    }
    out
}

pub async fn run_get_room(client: &AgentdClient, room_id: &str) -> String {
    let base = client.communicate_url();
    let room: Room = match client.inner.get(format!("{base}/rooms/{room_id}")).send().await {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(v) => v,
            Err(e) => return format!("Error parsing room: {e}"),
        },
        Ok(r) => return format!("Error: HTTP {} fetching room {room_id}", r.status()),
        Err(e) => return format!("Error: communicate unreachable at {base}: {e}"),
    };

    let participants: Vec<Participant> = match client
        .inner
        .get(format!("{base}/rooms/{room_id}/participants?limit=200"))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => match r.json::<Paginated<Participant>>().await {
            Ok(p) => p.items,
            Err(_) => Vec::new(),
        },
        _ => Vec::new(),
    };

    let icon = room_type_icon(&room.room_type);
    let mut out = format!("## Room: {} {} {}\n\n", icon, room.room_type, room.name);
    out.push_str(&format!("- **ID**: `{}`\n", room.id));
    if let Some(ref t) = room.topic {
        out.push_str(&format!("- **Topic**: {t}\n"));
    }
    if let Some(ref d) = room.description {
        out.push_str(&format!("- **Description**: {d}\n"));
    }
    out.push_str(&format!("- **Created by**: {}\n", room.created_by));
    out.push_str(&format!("- **Created**: {}\n", room.created_at));
    out.push_str(&format!("- **Updated**: {}\n\n", room.updated_at));

    if participants.is_empty() {
        out.push_str("### Participants\n\n_No participants._\n");
    } else {
        out.push_str(&format!("### Participants ({})\n\n", participants.len()));
        out.push_str("| Kind | Identifier | Display name | Role | Joined |\n");
        out.push_str("|------|------------|--------------|------|--------|\n");
        for p in &participants {
            out.push_str(&format!(
                "| {} {} | `{}` | {} | {} | {} |\n",
                sender_icon(&p.kind),
                p.kind,
                p.identifier,
                p.display_name,
                p.role,
                p.joined_at
            ));
        }
    }

    out
}

pub async fn run_list_messages(client: &AgentdClient, room_id: &str, limit: Option<u32>) -> String {
    let base = client.communicate_url();
    let limit_val = limit.unwrap_or(20).clamp(1, 200);
    let url = format!("{base}/rooms/{room_id}/messages?limit={limit_val}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!("Error: communicate unreachable at {base}: {e}");
        }
    };
    if !resp.status().is_success() {
        return format!("Error: HTTP {} listing messages for room {room_id}", resp.status());
    }
    let page: Paginated<Message> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing messages: {e}"),
    };

    if page.items.is_empty() {
        return format!("No messages in room `{room_id}`.");
    }

    let mut out = format!(
        "## Messages in room `{room_id}` — showing {} of {}\n\n",
        page.items.len(),
        page.total
    );
    out.push_str("| When | Sender | Status | Content |\n");
    out.push_str("|------|--------|--------|--------|\n");
    for m in &page.items {
        let sender = format!("{} {}", sender_icon(&m.sender_kind), m.sender_name);
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            m.created_at,
            sender,
            m.status,
            truncate(&m.content, 80)
        ));
    }
    out
}

pub async fn run_send_room_message(
    client: &AgentdClient,
    room_id: &str,
    sender_id: &str,
    sender_name: &str,
    sender_kind: &str,
    content: &str,
) -> String {
    let base = client.communicate_url();
    let url = format!("{base}/rooms/{room_id}/messages");

    let body = json!({
        "sender_id": sender_id,
        "sender_name": sender_name,
        "sender_kind": sender_kind,
        "content": content,
    });

    let resp = match client.inner.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: communicate unreachable at {base}: {e}"),
    };
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return format!("Error: HTTP {status} sending message: {body_text}");
    }
    let sent: Message = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing send response: {e}"),
    };
    format!(
        "✅ Message sent to room `{room_id}`\n\n- **Message ID**: `{}`\n- **Sender**: {} {} (`{}`)\n- **Created**: {}\n",
        sent.id, sender_icon(&sent.sender_kind), sent.sender_name, sent.sender_id, sent.created_at
    )
}
