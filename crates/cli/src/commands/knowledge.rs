//! Knowledgebase service command implementations.
//!
//! Provides CLI subcommands for managing per-project markdown documents via
//! the agentd-knowledge service, routed through the core gateway
//! (`AGENTD_CORE_SERVICE_URL`) with bearer-token authentication.
//!
//! # Available Commands
//!
//! - **list**: List documents for a project (paginated, optional prefix filter)
//! - **get**: Get document metadata by ID
//! - **content**: Get document metadata + markdown body by ID
//! - **create**: Create a new document (inline or from file)
//! - **update**: Update an existing document (inline or from file)
//! - **delete**: Delete a document by ID
//! - **gc**: Bulk-delete all documents for a project (garbage collect)
//! - **tree**: Display the virtual folder/file tree for a project
//! - **doctor**: Reconcile DB rows vs disk files; optionally fix divergences
//!
//! # Examples
//!
//! ```bash
//! agent knowledge list <project_id>
//! agent knowledge list <project_id> --prefix docs/
//! agent knowledge get <project_id> <doc_id>
//! agent knowledge content <project_id> <doc_id>
//! agent knowledge create <project_id> readme.md --content "# Hello"
//! agent knowledge create <project_id> readme.md --from-file ./README.md
//! agent knowledge update <project_id> <doc_id> --from-file ./README.md
//! agent knowledge delete <project_id> <doc_id>
//! agent knowledge gc <project_id>
//! agent knowledge tree <project_id>
//! agent knowledge doctor <project_id>
//! agent knowledge doctor <project_id> --fix
//! ```

use anyhow::Result;
use clap::Subcommand;
use colored::*;
use knowledge::client::KnowledgeClient;
use knowledge::types::{CreateDocumentRequest, UpdateDocumentRequest};
use prettytable::{format, Cell, Row, Table};

/// Knowledgebase service subcommands.
#[derive(Debug, Subcommand)]
pub enum KnowledgeCommand {
    /// Check the health of the knowledge service.
    Health,

    /// List documents for a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge list 550e8400-e29b-41d4-a716-446655440000
    /// agent knowledge list <project_id> --prefix docs/
    /// agent knowledge list <project_id> --limit 10 --offset 20
    /// ```
    List {
        /// Project UUID
        project_id: String,
        /// Filter to documents whose rel_path starts with this prefix
        #[arg(long)]
        prefix: Option<String>,
        /// Maximum number of results (default: 50)
        #[arg(long)]
        limit: Option<usize>,
        /// Pagination offset (default: 0)
        #[arg(long)]
        offset: Option<usize>,
    },

    /// Get document metadata by ID.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge get <project_id> <doc_id>
    /// ```
    Get {
        /// Project UUID
        project_id: String,
        /// Document UUID
        doc_id: String,
    },

    /// Get document metadata and markdown body by ID.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge content <project_id> <doc_id>
    /// ```
    Content {
        /// Project UUID
        project_id: String,
        /// Document UUID
        doc_id: String,
    },

    /// Create a new document.
    ///
    /// Provide body content via --content or --from-file (mutually exclusive).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge create <project_id> readme.md --content "# Hello"
    /// agent knowledge create <project_id> docs/api.md --from-file ./api.md --title "API Reference"
    /// ```
    Create {
        /// Project UUID
        project_id: String,
        /// Relative path (must end in .md, e.g. docs/readme.md)
        rel_path: String,
        /// Optional document title (defaults to filename stem)
        #[arg(long)]
        title: Option<String>,
        /// Inline markdown content
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        /// Read content from a file
        #[arg(long, conflicts_with = "content")]
        from_file: Option<std::path::PathBuf>,
    },

    /// Update an existing document.
    ///
    /// Provide new content via --content or --from-file. Use --expected-updated-at
    /// for optimistic concurrency control.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge update <project_id> <doc_id> --content "# Updated"
    /// agent knowledge update <project_id> <doc_id> --from-file ./updated.md
    /// ```
    Update {
        /// Project UUID
        project_id: String,
        /// Document UUID
        doc_id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// Inline markdown content
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        /// Read content from a file
        #[arg(long, conflicts_with = "content")]
        from_file: Option<std::path::PathBuf>,
        /// Optimistic concurrency token (RFC3339 updated_at from a prior GET)
        #[arg(long)]
        expected_updated_at: Option<String>,
    },

    /// Delete a document by ID.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge delete <project_id> <doc_id>
    /// ```
    Delete {
        /// Project UUID
        project_id: String,
        /// Document UUID
        doc_id: String,
    },

    /// Bulk-delete all documents for a project (garbage collect).
    ///
    /// Removes every document and file associated with the given project ID.
    /// This is irreversible. Pass `--yes` to skip the confirmation guard.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge gc <project_id> --yes
    /// ```
    Gc {
        /// Project UUID
        project_id: String,
        /// Skip the confirmation guard. Required for non-interactive / scripted use.
        #[arg(long)]
        yes: bool,
    },

    /// Display the virtual folder/file tree for a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge tree <project_id>
    /// ```
    Tree {
        /// Project UUID
        project_id: String,
    },

    /// Reconcile DB rows vs disk files for a project.
    ///
    /// Reports missing files (DB rows whose markdown file is absent from disk)
    /// and orphaned files (disk files with no DB row). Pass `--fix` to
    /// automatically delete the divergent entries.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent knowledge doctor <project_id>
    /// agent knowledge doctor <project_id> --fix
    /// ```
    Doctor {
        /// Project UUID
        project_id: String,
        /// Automatically fix divergences (delete stale DB rows + orphaned files).
        #[arg(long)]
        fix: bool,
    },
}

impl KnowledgeCommand {
    pub async fn execute(&self, client: &KnowledgeClient, json: bool) -> Result<()> {
        match self {
            KnowledgeCommand::Health => cmd_health(client, json).await,
            KnowledgeCommand::List { project_id, prefix, limit, offset } => {
                cmd_list(client, project_id, prefix.as_deref(), *limit, *offset, json).await
            }
            KnowledgeCommand::Get { project_id, doc_id } => {
                cmd_get(client, project_id, doc_id, json).await
            }
            KnowledgeCommand::Content { project_id, doc_id } => {
                cmd_content(client, project_id, doc_id, json).await
            }
            KnowledgeCommand::Create { project_id, rel_path, title, content, from_file } => {
                cmd_create(
                    client,
                    project_id,
                    rel_path,
                    title.as_deref(),
                    content.as_deref(),
                    from_file.as_ref(),
                    json,
                )
                .await
            }
            KnowledgeCommand::Update {
                project_id,
                doc_id,
                title,
                content,
                from_file,
                expected_updated_at,
            } => {
                let body = if content.is_some() || from_file.is_some() {
                    Some(resolve_content(content.as_deref(), from_file.as_ref())?)
                } else {
                    None
                };
                cmd_update(
                    client,
                    project_id,
                    doc_id,
                    title.as_deref(),
                    body,
                    expected_updated_at.as_deref(),
                    json,
                )
                .await
            }
            KnowledgeCommand::Delete { project_id, doc_id } => {
                cmd_delete(client, project_id, doc_id, json).await
            }
            KnowledgeCommand::Gc { project_id, yes } => {
                cmd_gc(client, project_id, *yes, json).await
            }
            KnowledgeCommand::Tree { project_id } => cmd_tree(client, project_id, json).await,
            KnowledgeCommand::Doctor { project_id, fix } => {
                cmd_doctor(client, project_id, *fix, json).await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

async fn cmd_health(client: &KnowledgeClient, json: bool) -> Result<()> {
    let status = client.health().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{}", "knowledge service is healthy".green());
    }
    Ok(())
}

async fn cmd_list(
    client: &KnowledgeClient,
    project_id: &str,
    prefix: Option<&str>,
    limit: Option<usize>,
    offset: Option<usize>,
    json: bool,
) -> Result<()> {
    let page = client.list_documents(project_id, prefix, limit, offset).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&page)?);
        return Ok(());
    }
    if page.items.is_empty() {
        println!("{}", "No documents found.".yellow());
        return Ok(());
    }
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(Row::new(vec![
        Cell::new("ID").style_spec("b"),
        Cell::new("Path").style_spec("b"),
        Cell::new("Title").style_spec("b"),
        Cell::new("Size").style_spec("b"),
        Cell::new("Updated").style_spec("b"),
    ]));
    for doc in &page.items {
        let short_id = doc.id.get(..8).unwrap_or(&doc.id);
        let updated = doc.updated_at.get(..19).unwrap_or(&doc.updated_at);
        table.add_row(Row::new(vec![
            Cell::new(short_id),
            Cell::new(&doc.rel_path),
            Cell::new(&doc.title),
            Cell::new(&format!("{} B", doc.size_bytes)),
            Cell::new(updated),
        ]));
    }
    table.printstd();
    println!("\n{} of {} document(s)", page.items.len(), page.total);
    Ok(())
}

async fn cmd_get(
    client: &KnowledgeClient,
    project_id: &str,
    doc_id: &str,
    json: bool,
) -> Result<()> {
    let doc = client.get_document(project_id, doc_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }
    println!("{}: {}", "ID".bold(), doc.id);
    println!("{}: {}", "Path".bold(), doc.rel_path);
    println!("{}: {}", "Title".bold(), doc.title);
    println!("{}: {} bytes", "Size".bold(), doc.size_bytes);
    println!("{}: {}", "Created".bold(), doc.created_at.get(..19).unwrap_or(&doc.created_at));
    println!("{}: {}", "Updated".bold(), doc.updated_at.get(..19).unwrap_or(&doc.updated_at));
    Ok(())
}

async fn cmd_content(
    client: &KnowledgeClient,
    project_id: &str,
    doc_id: &str,
    json: bool,
) -> Result<()> {
    let doc_content = client.get_document_content(project_id, doc_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc_content)?);
        return Ok(());
    }
    let doc = &doc_content.document;
    println!("{}: {}", "ID".bold(), doc.id);
    println!("{}: {}", "Path".bold(), doc.rel_path);
    println!("{}: {}", "Title".bold(), doc.title);
    println!("{}:", "Content".bold());
    println!("{}", doc_content.content);
    Ok(())
}

async fn cmd_create(
    client: &KnowledgeClient,
    project_id: &str,
    rel_path: &str,
    title: Option<&str>,
    content: Option<&str>,
    from_file: Option<&std::path::PathBuf>,
    json: bool,
) -> Result<()> {
    let body = resolve_content(content, from_file)?;
    let req = CreateDocumentRequest {
        rel_path: rel_path.to_string(),
        title: title.map(str::to_string),
        content: body,
    };
    let doc = client.create_document(project_id, req).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }
    println!("{} {}", "Created document:".green(), doc.id);
    println!("  {} {}", "Path:".bold(), doc.rel_path);
    println!("  {} {}", "Title:".bold(), doc.title);
    println!("  {} {} bytes", "Size:".bold(), doc.size_bytes);
    Ok(())
}

async fn cmd_update(
    client: &KnowledgeClient,
    project_id: &str,
    doc_id: &str,
    title: Option<&str>,
    body: Option<String>,
    expected_updated_at: Option<&str>,
    json: bool,
) -> Result<()> {
    let req = UpdateDocumentRequest {
        content: body,
        title: title.map(str::to_string),
        expected_updated_at: expected_updated_at.map(str::to_string),
    };
    let doc = client.update_document(project_id, doc_id, req).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(());
    }
    println!("{} {}", "Updated document:".green(), doc.id);
    println!("  {} {}", "Path:".bold(), doc.rel_path);
    println!("  {} {}", "Title:".bold(), doc.title);
    println!("  {} {} bytes", "Size:".bold(), doc.size_bytes);
    Ok(())
}

async fn cmd_delete(
    client: &KnowledgeClient,
    project_id: &str,
    doc_id: &str,
    json: bool,
) -> Result<()> {
    client.delete_document(project_id, doc_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "deleted": doc_id }))?);
    } else {
        println!("{} {}", "Deleted document".green(), doc_id);
    }
    Ok(())
}

async fn cmd_gc(client: &KnowledgeClient, project_id: &str, yes: bool, json: bool) -> Result<()> {
    if !yes {
        anyhow::bail!(
            "This will permanently delete all documents for project {project_id}. \
             Pass --yes to confirm."
        );
    }
    client.bulk_delete_documents(project_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "gc": project_id }))?);
    } else {
        println!("{} all documents for project {}", "Deleted".green(), project_id);
    }
    Ok(())
}

async fn cmd_tree(client: &KnowledgeClient, project_id: &str, json: bool) -> Result<()> {
    let tree = client.get_tree(project_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
        return Ok(());
    }
    if tree.is_empty() {
        println!("{}", "No documents found.".yellow());
        return Ok(());
    }
    print_tree_nodes(&tree, "");
    Ok(())
}

async fn cmd_doctor(
    client: &KnowledgeClient,
    project_id: &str,
    fix: bool,
    json: bool,
) -> Result<()> {
    let report =
        if fix { client.doctor_fix(project_id).await? } else { client.doctor(project_id).await? };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.missing_files.is_empty() && report.orphaned_files.is_empty() {
        println!("{}", "No divergences found.".green());
        return Ok(());
    }
    if !report.missing_files.is_empty() {
        println!("{} (DB rows with no file on disk):", "Missing files".bold().yellow());
        for f in &report.missing_files {
            println!("  - {f}");
        }
    }
    if !report.orphaned_files.is_empty() {
        println!("{} (disk files with no DB row):", "Orphaned files".bold().yellow());
        for f in &report.orphaned_files {
            println!("  - {f}");
        }
    }
    if fix {
        println!("\n{} {} issue(s) fixed.", "Doctor:".bold().green(), report.fixed);
    } else {
        println!("\n{} Run with --fix to repair automatically.", "Hint:".bold());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve content from inline string or file path.
fn resolve_content(
    content: Option<&str>,
    from_file: Option<&std::path::PathBuf>,
) -> Result<String> {
    if let Some(c) = content {
        return Ok(c.to_string());
    }
    if let Some(path) = from_file {
        return std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()));
    }
    Ok(String::new())
}

/// Recursively print tree nodes.
fn print_tree_nodes(nodes: &[knowledge::types::TreeNode], indent: &str) {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_indent =
            if is_last { format!("{indent}    ") } else { format!("{indent}│   ") };
        match node {
            knowledge::types::TreeNode::Folder { name, children, .. } => {
                println!("{indent}{connector}{}", name.bold().blue());
                print_tree_nodes(children, &child_indent);
            }
            knowledge::types::TreeNode::File { name, .. } => {
                println!("{indent}{connector}{name}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_content_inline() {
        let content = resolve_content(Some("# Hello"), None).unwrap();
        assert_eq!(content, "# Hello");
    }

    #[test]
    fn test_resolve_content_empty() {
        let content = resolve_content(None, None).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_resolve_content_from_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"# From file").unwrap();
        let content = resolve_content(None, Some(&tmp.path().to_path_buf())).unwrap();
        assert_eq!(content, "# From file");
    }
}
