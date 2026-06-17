//! Knowledgebase tools for the agentd-knowledge service.
//!
//! These tools let MCP clients (agents) browse, read, and author per-project
//! markdown documents stored by the knowledge service. They mirror the
//! `agent knowledge` CLI subcommands but return markdown-formatted summaries
//! suited for inline agent consumption.
//!
//! Like the other MCP tool modules, these call the knowledge service directly
//! on its localhost port (`AGENTD_KNOWLEDGE_URL`, default `:17011`) rather than
//! through the core gateway — the MCP server is a trusted local component.

use crate::client::AgentdClient;
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Response types (subset of `knowledge::types` we render)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Document {
    id: String,
    rel_path: String,
    title: String,
    #[serde(default)]
    size_bytes: i64,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct DocumentContent {
    document: Document,
    content: String,
}

#[derive(Debug, Deserialize)]
struct PaginatedDocuments {
    items: Vec<Document>,
    total: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum TreeNode {
    Folder { name: String, children: Vec<TreeNode> },
    File { name: String, doc_id: String },
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Trim an RFC3339 timestamp to `YYYY-MM-DDTHH:MM:SS` for compact display.
fn short_ts(ts: &str) -> &str {
    ts.get(..19).unwrap_or(ts)
}

/// First 8 characters of a UUID, for compact tables.
fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn document_summary(doc: &Document) -> String {
    let mut out = format!("## Document `{}`\n\n", doc.id);
    out.push_str(&format!("- **Path**: `{}`\n", doc.rel_path));
    out.push_str(&format!("- **Title**: {}\n", doc.title));
    out.push_str(&format!("- **Size**: {} bytes\n", doc.size_bytes));
    if !doc.created_at.is_empty() {
        out.push_str(&format!("- **Created**: {}\n", short_ts(&doc.created_at)));
    }
    if !doc.updated_at.is_empty() {
        out.push_str(&format!("- **Updated**: {}\n", short_ts(&doc.updated_at)));
    }
    out
}

fn render_tree(nodes: &[TreeNode], indent: &str, out: &mut String) {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_indent =
            if is_last { format!("{indent}    ") } else { format!("{indent}│   ") };
        match node {
            TreeNode::Folder { name, children } => {
                out.push_str(&format!("{indent}{connector}📁 {name}\n"));
                render_tree(children, &child_indent, out);
            }
            TreeNode::File { name, doc_id } => {
                out.push_str(&format!("{indent}{connector}📄 {name}  `{}`\n", short_id(doc_id)));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

pub async fn run_list_documents(
    client: &AgentdClient,
    project_id: &str,
    prefix: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> String {
    let base = client.knowledge_url();
    let mut url = format!("{base}/projects/{project_id}/documents");
    let mut params: Vec<String> = Vec::new();
    if let Some(p) = prefix {
        params.push(format!("prefix={}", urlencode(p)));
    }
    if let Some(l) = limit {
        params.push(format!("limit={}", l.clamp(1, 500)));
    }
    if let Some(o) = offset {
        params.push(format!("offset={o}"));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: knowledge returned HTTP {status}: {text}");
    }
    let page: PaginatedDocuments = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing list response: {e}"),
    };

    if page.items.is_empty() {
        return format!("No documents found for project `{project_id}`.");
    }

    let mut out = format!(
        "## Documents in project `{project_id}` — showing {} of {}\n\n",
        page.items.len(),
        page.total
    );
    out.push_str("| ID | Path | Title | Size | Updated |\n");
    out.push_str("|----|------|-------|------|---------|\n");
    for d in &page.items {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} B | {} |\n",
            short_id(&d.id),
            d.rel_path,
            d.title,
            d.size_bytes,
            short_ts(&d.updated_at),
        ));
    }
    out
}

pub async fn run_read_document(client: &AgentdClient, project_id: &str, doc_id: &str) -> String {
    let base = client.knowledge_url();
    let url = format!("{base}/projects/{project_id}/documents/{doc_id}/content");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Document `{doc_id}` not found in project `{project_id}`.");
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: knowledge returned HTTP {status}: {text}");
    }
    let dc: DocumentContent = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing document content: {e}"),
    };

    let mut out = document_summary(&dc.document);
    out.push_str("\n### Content\n\n");
    out.push_str(&dc.content);
    out.push('\n');
    out
}

pub async fn run_create_document(
    client: &AgentdClient,
    project_id: &str,
    rel_path: &str,
    content: &str,
    title: Option<&str>,
) -> String {
    let base = client.knowledge_url();
    let url = format!("{base}/projects/{project_id}/documents");

    let mut body = json!({ "rel_path": rel_path, "content": content });
    if let Some(t) = title {
        body["title"] = json!(t);
    }

    let resp = match client.inner.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: create failed with HTTP {status}: {text}");
    }
    let doc: Document = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing create response: {e}"),
    };

    format!("✅ Created document.\n\n{}", document_summary(&doc))
}

pub async fn run_update_document(
    client: &AgentdClient,
    project_id: &str,
    doc_id: &str,
    content: Option<&str>,
    title: Option<&str>,
    expected_updated_at: Option<&str>,
) -> String {
    if content.is_none() && title.is_none() {
        return "Error: provide `content` and/or `title` to update.".to_string();
    }
    let base = client.knowledge_url();
    let url = format!("{base}/projects/{project_id}/documents/{doc_id}");

    let mut body = json!({});
    if let Some(c) = content {
        body["content"] = json!(c);
    }
    if let Some(t) = title {
        body["title"] = json!(t);
    }
    if let Some(e) = expected_updated_at {
        body["expected_updated_at"] = json!(e);
    }

    let resp = match client.inner.put(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Document `{doc_id}` not found in project `{project_id}`.");
    }
    if resp.status() == reqwest::StatusCode::CONFLICT {
        return "Error: update rejected — the document changed since `expected_updated_at` \
                (optimistic concurrency conflict). Re-read it and retry."
            .to_string();
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: update failed with HTTP {status}: {text}");
    }
    let doc: Document = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing update response: {e}"),
    };

    format!("✅ Updated document.\n\n{}", document_summary(&doc))
}

pub async fn run_delete_document(client: &AgentdClient, project_id: &str, doc_id: &str) -> String {
    let base = client.knowledge_url();
    let url = format!("{base}/projects/{project_id}/documents/{doc_id}");

    let resp = match client.inner.delete(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return format!("Document `{doc_id}` not found in project `{project_id}`.");
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: delete failed with HTTP {status}: {text}");
    }
    format!("✅ Deleted document `{doc_id}` from project `{project_id}`.")
}

pub async fn run_get_tree(client: &AgentdClient, project_id: &str) -> String {
    let base = client.knowledge_url();
    let url = format!("{base}/projects/{project_id}/tree");

    let resp = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("Error: knowledge service unreachable at {base}: {e}"),
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return format!("Error: knowledge returned HTTP {status}: {text}");
    }
    let tree: Vec<TreeNode> = match resp.json().await {
        Ok(v) => v,
        Err(e) => return format!("Error parsing tree: {e}"),
    };

    if tree.is_empty() {
        return format!("No documents found for project `{project_id}`.");
    }
    let mut out = format!("## Document tree for project `{project_id}`\n\n```\n");
    render_tree(&tree, "", &mut out);
    out.push_str("```\n");
    out
}

// ---------------------------------------------------------------------------
// URL encoding helper (query-parameter values)
// ---------------------------------------------------------------------------

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_id_and_ts() {
        assert_eq!(short_id("550e8400-e29b-41d4-a716-446655440000"), "550e8400");
        assert_eq!(short_id("abc"), "abc");
        assert_eq!(short_ts("2026-06-17T12:34:56.789Z"), "2026-06-17T12:34:56");
        assert_eq!(short_ts("short"), "short");
    }

    #[test]
    fn test_urlencode_query_value() {
        assert_eq!(urlencode("docs/api.md"), "docs%2Fapi.md");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("plain-_.~"), "plain-_.~");
    }

    #[test]
    fn test_document_summary_omits_empty_timestamps() {
        let doc = Document {
            id: "id123456789".to_string(),
            rel_path: "docs/readme.md".to_string(),
            title: "Readme".to_string(),
            size_bytes: 42,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let out = document_summary(&doc);
        assert!(out.contains("docs/readme.md"));
        assert!(out.contains("42 bytes"));
        assert!(!out.contains("Created"));
        assert!(!out.contains("Updated"));
    }

    #[test]
    fn test_render_tree_nests_folders_and_files() {
        let tree = vec![TreeNode::Folder {
            name: "docs".to_string(),
            children: vec![TreeNode::File {
                name: "api.md".to_string(),
                doc_id: "deadbeef-0000".to_string(),
            }],
        }];
        let mut out = String::new();
        render_tree(&tree, "", &mut out);
        assert!(out.contains("📁 docs"));
        assert!(out.contains("📄 api.md"));
        assert!(out.contains("deadbeef"));
    }

    #[test]
    fn test_tree_node_deserializes_tagged_json() {
        let json = serde_json::json!({
            "type": "folder",
            "name": "docs",
            "path": "docs",
            "children": [
                { "type": "file", "name": "a.md", "path": "docs/a.md", "doc_id": "id-1" }
            ]
        });
        let node: TreeNode = serde_json::from_value(json).unwrap();
        match node {
            TreeNode::Folder { name, children } => {
                assert_eq!(name, "docs");
                assert_eq!(children.len(), 1);
            }
            _ => panic!("expected folder"),
        }
    }
}
