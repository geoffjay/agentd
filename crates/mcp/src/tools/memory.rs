//! Memory search, listing, and retrieval tools for the agentd memory service.
//!
//! These tools let MCP clients query stored agent memories (information,
//! questions, requests) and inspect individual records.

use crate::client::AgentdClient;
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Memory {
    id: String,
    content: String,
    #[serde(rename = "type")]
    mem_type: String,
    #[serde(default)]
    tags: Vec<String>,
    created_by: String,
    #[serde(default)]
    owner: Option<String>,
    created_at: String,
    updated_at: String,
    visibility: String,
    #[serde(default)]
    shared_with: Vec<String>,
    #[serde(default)]
    references: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    memories: Vec<Memory>,
    total: u64,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    items: Vec<Memory>,
    total: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn type_icon(t: &str) -> &'static str {
    match t {
        "information" => "📘",
        "question" => "❓",
        "request" => "📨",
        _ => "📝",
    }
}

fn visibility_icon(v: &str) -> &'static str {
    match v {
        "public" => "🌐",
        "private" => "🔒",
        "shared" => "🤝",
        _ => "❔",
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

fn format_memory_row(m: &Memory) -> String {
    let tags = if m.tags.is_empty() { "-".to_string() } else { m.tags.join(", ") };
    format!(
        "| {} {} | {} | `{}` | {} | {} | {} |\n",
        type_icon(&m.mem_type),
        m.mem_type,
        visibility_icon(&m.visibility),
        m.id,
        m.created_by,
        tags,
        truncate(&m.content, 60)
    )
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

pub async fn run_search_memories(
    client: &AgentdClient,
    query: &str,
    tags: Option<Vec<String>>,
    mem_type: Option<&str>,
    limit: Option<u32>,
) -> String {
    let base = client.memory_url();
    let url = format!("{base}/memories/search");
    let limit_val = limit.unwrap_or(10).clamp(1, 100);

    let mut body = json!({
        "query": query,
        "limit": limit_val,
    });
    if let Some(t) = tags {
        body["tags"] = json!(t);
    }
    if let Some(t) = mem_type {
        body["type"] = json!(t);
    }

    let resp = match client.inner.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: memory service unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: memory returned HTTP {status}: {text}");
    }
    let result: SearchResponse = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing search response: {e}"),
    };

    if result.memories.is_empty() {
        return format!("No memories found matching `{query}`.");
    }

    let mut out = format!(
        "## Memory search: `{query}` — {} results (total {})\n\n",
        result.memories.len(),
        result.total
    );
    out.push_str("| Type | Visibility | ID | Created by | Tags | Content |\n");
    out.push_str("|------|------------|-----|-----------|------|---------|\n");
    for m in &result.memories {
        out.push_str(&format_memory_row(m));
    }
    out
}

pub async fn run_list_memories(
    client: &AgentdClient,
    mem_type: Option<&str>,
    tag: Option<&str>,
    created_by: Option<&str>,
    visibility: Option<&str>,
    limit: Option<u32>,
) -> String {
    let base = client.memory_url();
    let limit_val = limit.unwrap_or(50).clamp(1, 200);
    let mut url = format!("{base}/memories?limit={limit_val}");
    if let Some(t) = mem_type {
        url.push_str(&format!("&type={t}"));
    }
    if let Some(t) = tag {
        url.push_str(&format!("&tag={t}"));
    }
    if let Some(c) = created_by {
        url.push_str(&format!("&created_by={c}"));
    }
    if let Some(v) = visibility {
        url.push_str(&format!("&visibility={v}"));
    }

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: memory unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        return format!("Error: HTTP {} listing memories", resp.status());
    }
    let page: ListResponse = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing memories: {e}"),
    };

    if page.items.is_empty() {
        return "No memories found.".to_string();
    }

    let mut out = format!("## Memories — showing {} of {}\n\n", page.items.len(), page.total);
    out.push_str("| Type | Visibility | ID | Created by | Tags | Content |\n");
    out.push_str("|------|------------|-----|-----------|------|---------|\n");
    for m in &page.items {
        out.push_str(&format_memory_row(m));
    }
    out
}

pub async fn run_get_memory(client: &AgentdClient, memory_id: &str) -> String {
    let base = client.memory_url();
    let url = format!("{base}/memories/{memory_id}");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: memory unreachable at {base}: {e}"),
    };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Memory `{memory_id}` not found.");
    }
    if !resp.status().is_success() {
        return format!("Error: HTTP {} fetching memory", resp.status());
    }
    let m: Memory = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing memory: {e}"),
    };

    let tags = if m.tags.is_empty() { "-".to_string() } else { m.tags.join(", ") };
    let mut out = format!("## Memory `{}`\n\n", m.id);
    out.push_str(&format!("- **Type**: {} {}\n", type_icon(&m.mem_type), m.mem_type));
    out.push_str(&format!(
        "- **Visibility**: {} {}\n",
        visibility_icon(&m.visibility),
        m.visibility
    ));
    out.push_str(&format!("- **Created by**: {}\n", m.created_by));
    if let Some(ref o) = m.owner {
        out.push_str(&format!("- **Owner**: {o}\n"));
    }
    out.push_str(&format!("- **Tags**: {tags}\n"));
    out.push_str(&format!("- **Created**: {}\n", m.created_at));
    out.push_str(&format!("- **Updated**: {}\n", m.updated_at));
    if !m.shared_with.is_empty() {
        out.push_str(&format!("- **Shared with**: {}\n", m.shared_with.join(", ")));
    }
    if !m.references.is_empty() {
        out.push_str(&format!("- **References**: {}\n", m.references.join(", ")));
    }
    out.push_str("\n### Content\n\n");
    out.push_str(&m.content);
    out.push('\n');
    out
}
