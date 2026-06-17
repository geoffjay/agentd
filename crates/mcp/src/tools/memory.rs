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
// Validation
// ---------------------------------------------------------------------------

/// Memory type values accepted by the service (`MemoryType`, lowercase).
const MEMORY_TYPES: &[&str] = &["information", "question", "request"];
/// Visibility values accepted by the service (`VisibilityLevel`, lowercase).
const VISIBILITY_LEVELS: &[&str] = &["public", "private", "shared"];

/// Validate create-memory inputs client-side, mirroring the service's own
/// checks so callers get a clear error before the round-trip. Returns the
/// resolved visibility (defaulting to `public`).
fn validate_create_memory<'a>(
    content: &str,
    created_by: &str,
    mem_type: Option<&str>,
    visibility: Option<&'a str>,
    shared_with: &[String],
) -> Result<&'a str, String> {
    if content.trim().is_empty() {
        return Err("🔴 `content` must not be empty.".to_string());
    }
    if created_by.trim().is_empty() {
        return Err("🔴 `created_by` must not be empty — pass the storing actor's identity \
                    (e.g. your agent name)."
            .to_string());
    }
    if let Some(t) = mem_type {
        if !MEMORY_TYPES.contains(&t) {
            return Err(format!(
                "🔴 Unknown memory type `{t}`. Valid types: {}.",
                MEMORY_TYPES.join(", ")
            ));
        }
    }
    let visibility = visibility.unwrap_or("public");
    if !VISIBILITY_LEVELS.contains(&visibility) {
        return Err(format!(
            "🔴 Unknown visibility `{visibility}`. Valid levels: {}.",
            VISIBILITY_LEVELS.join(", ")
        ));
    }
    if visibility == "shared" && shared_with.is_empty() {
        return Err("🔴 `visibility` is `shared` but `shared_with` is empty — list the actors \
                    allowed to read this memory."
            .to_string());
    }
    Ok(visibility)
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

/// Create (store) a new memory via `POST /memories`.
#[allow(clippy::too_many_arguments)]
pub async fn run_create_memory(
    client: &AgentdClient,
    content: &str,
    created_by: &str,
    mem_type: Option<&str>,
    tags: Option<Vec<String>>,
    visibility: Option<&str>,
    shared_with: Option<Vec<String>>,
    references: Option<Vec<String>>,
) -> String {
    let shared = shared_with.unwrap_or_default();
    let visibility =
        match validate_create_memory(content, created_by, mem_type, visibility, &shared) {
            Ok(v) => v,
            Err(msg) => return msg,
        };

    let base = client.memory_url();
    let url = format!("{base}/memories");

    let mut body = json!({
        "content": content,
        "created_by": created_by,
        "visibility": visibility,
    });
    if let Some(t) = mem_type {
        body["type"] = json!(t);
    }
    if let Some(tags) = tags {
        body["tags"] = json!(tags);
    }
    if !shared.is_empty() {
        body["shared_with"] = json!(shared);
    }
    if let Some(refs) = references {
        body["references"] = json!(refs);
    }

    let resp = match client.inner.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: memory service unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("🔴 Failed to store memory (HTTP {status}): {text}");
    }
    let m: Memory = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing create response: {e}"),
    };

    let tags = if m.tags.is_empty() { "-".to_string() } else { m.tags.join(", ") };
    format!(
        "✅ Stored memory `{}`.\n\
         - **Type**: {} {}\n\
         - **Visibility**: {} {}\n\
         - **Created by**: {}\n\
         - **Tags**: {}",
        m.id,
        type_icon(&m.mem_type),
        m.mem_type,
        visibility_icon(&m.visibility),
        m.visibility,
        m.created_by,
        tags,
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_create_memory_defaults_visibility_to_public() {
        let v = validate_create_memory("hello", "agent-1", None, None, &[]).unwrap();
        assert_eq!(v, "public");
    }

    #[test]
    fn validate_create_memory_rejects_empty_content() {
        let err = validate_create_memory("  ", "agent-1", None, None, &[]).unwrap_err();
        assert!(err.contains("content"), "{err}");
    }

    #[test]
    fn validate_create_memory_rejects_empty_created_by() {
        let err = validate_create_memory("hello", "", None, None, &[]).unwrap_err();
        assert!(err.contains("created_by"), "{err}");
    }

    #[test]
    fn validate_create_memory_rejects_unknown_type() {
        let err = validate_create_memory("hello", "agent-1", Some("rumor"), None, &[]).unwrap_err();
        assert!(err.contains("rumor"), "{err}");
        assert!(err.contains("information"), "lists valid types: {err}");
    }

    #[test]
    fn validate_create_memory_rejects_unknown_visibility() {
        let err =
            validate_create_memory("hello", "agent-1", None, Some("secret"), &[]).unwrap_err();
        assert!(err.contains("secret"), "{err}");
    }

    #[test]
    fn validate_create_memory_requires_shared_with_when_shared() {
        let err =
            validate_create_memory("hello", "agent-1", None, Some("shared"), &[]).unwrap_err();
        assert!(err.contains("shared_with"), "{err}");

        let ok = validate_create_memory(
            "hello",
            "agent-1",
            Some("information"),
            Some("shared"),
            &["ops-agent".to_string()],
        );
        assert_eq!(ok.unwrap(), "shared");
    }
}
