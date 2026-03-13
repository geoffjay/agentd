//! Memory service command implementations.
//!
//! Provides CLI subcommands for interacting with the agentd-memory service,
//! following berry-rs's CLI patterns (`remember`, `recall`, `search`, `forget`)
//! adapted to agentd's conventions.
//!
//! # Available Commands
//!
//! - **health**: Check the health of the memory service
//! - **remember**: Store a new memory record
//! - **recall**: Retrieve a specific memory by ID
//! - **search**: Semantic similarity search
//! - **forget**: Delete a memory record
//! - **list**: List memories with optional filters
//! - **visibility**: Update visibility and share list
//!
//! # Examples
//!
//! ```bash
//! # Store a memory
//! agent memory remember "Paris is the capital of France." \
//!   --created-by agent-1 --type information --tags geography,europe
//!
//! # Semantic search
//! agent memory search "capital of France" --limit 5
//!
//! # Recall a specific memory
//! agent memory recall mem_1234567890_abcdef01
//!
//! # Delete a memory
//! agent memory forget mem_1234567890_abcdef01
//!
//! # List with filters
//! agent memory list --type question --visibility public --limit 20
//!
//! # Update visibility
//! agent memory visibility mem_1234567890_abcdef01 shared --share-with agent-2,agent-3
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use memory::client::MemoryClient;
use memory::types::*;
use prettytable::{format, Cell, Row, Table};

/// Subcommands for the agentd-memory service.
///
/// Manage agent memories: store, search, recall, and delete records with
/// semantic search powered by LanceDB vector embeddings.
#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Check the health of the memory service.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory health
    /// ```
    Health,

    /// Store a new memory record.
    ///
    /// Creates a new memory with an embedding vector for semantic search.
    /// Only `content` and `--created-by` are required; all other fields have
    /// sensible defaults.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory remember "Paris is the capital of France." --created-by agent-1
    /// agent memory remember "How do I reset my password?" \
    ///   --created-by user-42 --type question --tags auth,help \
    ///   --visibility shared --share-with agent-support
    /// ```
    Remember {
        /// The natural-language content to store.
        content: String,

        /// Identity of the actor creating this memory.
        #[arg(long)]
        created_by: String,

        /// Memory type: information, question, or request.
        #[arg(long, rename_all = "lower", default_value = "information")]
        r#type: String,

        /// Comma-separated tags for filtering.
        #[arg(long)]
        tags: Option<String>,

        /// Visibility level: public, shared, or private.
        #[arg(long, default_value = "public")]
        visibility: String,

        /// Comma-separated actor IDs to share with (used with --visibility shared).
        #[arg(long)]
        share_with: Option<String>,

        /// Comma-separated reference IDs of related memories.
        #[arg(long)]
        references: Option<String>,
    },

    /// Retrieve a specific memory by ID.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory recall mem_1234567890_abcdef01
    /// agent memory recall mem_1234567890_abcdef01 --json
    /// ```
    Recall {
        /// The memory ID to retrieve.
        id: String,
    },

    /// Semantic similarity search over stored memories.
    ///
    /// Embeds the query text and finds the most similar memories using
    /// vector similarity search.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory search "capital of France"
    /// agent memory search "password reset" --type question --limit 5
    /// agent memory search "deployment steps" --as-actor agent-1 --tags devops
    /// ```
    Search {
        /// Natural-language query for similarity search.
        query: String,

        /// Actor performing the search (controls visibility filtering).
        #[arg(long)]
        as_actor: Option<String>,

        /// Filter by memory type.
        #[arg(long, rename_all = "lower")]
        r#type: Option<String>,

        /// Comma-separated tags to filter by.
        #[arg(long)]
        tags: Option<String>,

        /// Maximum number of results (default: 10).
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Only return memories created on or after this date (RFC3339).
        #[arg(long)]
        since: Option<String>,

        /// Only return memories created on or before this date (RFC3339).
        #[arg(long)]
        until: Option<String>,
    },

    /// Delete a memory record.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory forget mem_1234567890_abcdef01
    /// ```
    Forget {
        /// The memory ID to delete.
        id: String,
    },

    /// List memories with optional filters and pagination.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory list
    /// agent memory list --type question --limit 20
    /// agent memory list --created-by agent-1 --visibility private
    /// agent memory list --tag auth --limit 5 --offset 10
    /// ```
    List {
        /// Filter by memory type.
        #[arg(long, rename_all = "lower")]
        r#type: Option<String>,

        /// Filter by tag.
        #[arg(long)]
        tag: Option<String>,

        /// Filter by creator identity.
        #[arg(long)]
        created_by: Option<String>,

        /// Filter by visibility level.
        #[arg(long)]
        visibility: Option<String>,

        /// Maximum number of items to return (default: 50).
        #[arg(long, default_value = "50")]
        limit: usize,

        /// Number of items to skip for pagination.
        #[arg(long, default_value = "0")]
        offset: usize,
    },

    /// Update the visibility level and share list of a memory.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent memory visibility mem_123_abc public
    /// agent memory visibility mem_123_abc shared --share-with agent-2,agent-3
    /// agent memory visibility mem_123_abc private
    /// ```
    Visibility {
        /// The memory ID to update.
        id: String,

        /// New visibility level: public, shared, or private.
        level: String,

        /// Comma-separated actor IDs to share with (required for "shared" level).
        #[arg(long)]
        share_with: Option<String>,
    },
}

impl MemoryCommand {
    /// Execute the memory subcommand using the provided client.
    pub async fn execute(&self, client: &MemoryClient, json: bool) -> Result<()> {
        match self {
            MemoryCommand::Health => memory_health(client, json).await,
            MemoryCommand::Remember {
                content,
                created_by,
                r#type,
                tags,
                visibility,
                share_with,
                references,
            } => {
                remember(
                    client,
                    content,
                    created_by,
                    r#type,
                    tags.as_deref(),
                    visibility,
                    share_with.as_deref(),
                    references.as_deref(),
                    json,
                )
                .await
            }
            MemoryCommand::Recall { id } => recall(client, id, json).await,
            MemoryCommand::Search {
                query,
                as_actor,
                r#type,
                tags,
                limit,
                since,
                until,
            } => {
                search(
                    client,
                    query,
                    as_actor.as_deref(),
                    r#type.as_deref(),
                    tags.as_deref(),
                    *limit,
                    since.as_deref(),
                    until.as_deref(),
                    json,
                )
                .await
            }
            MemoryCommand::Forget { id } => forget(client, id, json).await,
            MemoryCommand::List {
                r#type,
                tag,
                created_by,
                visibility,
                limit,
                offset,
            } => {
                list(
                    client,
                    r#type.as_deref(),
                    tag.as_deref(),
                    created_by.as_deref(),
                    visibility.as_deref(),
                    *limit,
                    *offset,
                    json,
                )
                .await
            }
            MemoryCommand::Visibility {
                id,
                level,
                share_with,
            } => update_visibility(client, id, level, share_with.as_deref(), json).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn memory_health(client: &MemoryClient, json: bool) -> Result<()> {
    let body = client.health().await.context("Failed to reach memory service")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        let status = body["status"].as_str().unwrap_or("unknown");
        let version = body["version"].as_str().unwrap_or("unknown");
        let vector_ok = body["details"]["vector_store"]
            .as_bool()
            .unwrap_or(false);

        println!(
            "{} agentd-memory {} (v{})",
            "✅".green(),
            status.green(),
            version
        );
        if vector_ok {
            println!("   Vector store: {}", "healthy".green());
        } else {
            println!("   Vector store: {}", "unhealthy".red());
        }
    }
    Ok(())
}

async fn remember(
    client: &MemoryClient,
    content: &str,
    created_by: &str,
    memory_type: &str,
    tags: Option<&str>,
    visibility: &str,
    share_with: Option<&str>,
    references: Option<&str>,
    json: bool,
) -> Result<()> {
    let memory_type = memory_type
        .parse::<MemoryType>()
        .map_err(|e| anyhow::anyhow!("Invalid memory type: {e}"))?;
    let visibility = visibility
        .parse::<VisibilityLevel>()
        .map_err(|e| anyhow::anyhow!("Invalid visibility level: {e}"))?;

    let tags_vec: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let shared_vec: Vec<String> = share_with
        .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let refs_vec: Vec<String> = references
        .map(|r| r.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let request = CreateMemoryRequest {
        content: content.to_string(),
        memory_type,
        tags: tags_vec,
        created_by: created_by.to_string(),
        references: refs_vec,
        visibility,
        shared_with: shared_vec,
    };

    let memory = client
        .create_memory(&request)
        .await
        .context("Failed to create memory")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!("{}", "Memory created successfully!".green().bold());
        display_memory(&memory);
    }
    Ok(())
}

async fn recall(client: &MemoryClient, id: &str, json: bool) -> Result<()> {
    let memory = client
        .get_memory(id)
        .await
        .context(format!("Failed to retrieve memory '{id}'"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        display_memory(&memory);
    }
    Ok(())
}

async fn search(
    client: &MemoryClient,
    query: &str,
    as_actor: Option<&str>,
    memory_type: Option<&str>,
    tags: Option<&str>,
    limit: usize,
    since: Option<&str>,
    until: Option<&str>,
    json: bool,
) -> Result<()> {
    let memory_type = memory_type
        .map(|t| {
            t.parse::<MemoryType>()
                .map_err(|e| anyhow::anyhow!("Invalid memory type: {e}"))
        })
        .transpose()?;

    let tags_vec: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let from = since
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|e| anyhow::anyhow!("Invalid --since date (expected RFC3339): {e}"))
        })
        .transpose()?;

    let to = until
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|e| anyhow::anyhow!("Invalid --until date (expected RFC3339): {e}"))
        })
        .transpose()?;

    let request = SearchRequest {
        query: query.to_string(),
        as_actor: as_actor.map(String::from),
        memory_type,
        tags: tags_vec,
        from,
        to,
        limit,
    };

    let response = client
        .search_memories(&request)
        .await
        .context("Search failed")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!(
            "Found {} result{}",
            response.total.to_string().cyan(),
            if response.total == 1 { "" } else { "s" }
        );

        if response.memories.is_empty() {
            println!("{}", "No matching memories found.".yellow());
        } else {
            println!("{}", "═".repeat(80).cyan());
            for memory in &response.memories {
                display_memory_brief(memory);
                println!("{}", "─".repeat(80).dimmed());
            }
        }
    }
    Ok(())
}

async fn forget(client: &MemoryClient, id: &str, json: bool) -> Result<()> {
    let response = client
        .delete_memory(id)
        .await
        .context(format!("Failed to delete memory '{id}'"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.deleted {
        println!("{} Memory {} deleted", "✅".green(), id.cyan());
    } else {
        println!("{} Memory {} not found", "⚠️".yellow(), id.cyan());
    }
    Ok(())
}

async fn list(
    client: &MemoryClient,
    memory_type: Option<&str>,
    tag: Option<&str>,
    created_by: Option<&str>,
    visibility: Option<&str>,
    limit: usize,
    offset: usize,
    json: bool,
) -> Result<()> {
    // Validate filter values before sending
    if let Some(t) = memory_type {
        t.parse::<MemoryType>()
            .map_err(|e| anyhow::anyhow!("Invalid memory type: {e}"))?;
    }
    if let Some(v) = visibility {
        v.parse::<VisibilityLevel>()
            .map_err(|e| anyhow::anyhow!("Invalid visibility level: {e}"))?;
    }

    let response = client
        .list_memories(
            memory_type,
            tag,
            created_by,
            visibility,
            Some(limit),
            Some(offset),
        )
        .await
        .context("Failed to list memories")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!(
            "Showing {}/{} memories (offset {})",
            response.items.len().to_string().cyan(),
            response.total.to_string().cyan(),
            offset
        );

        if response.items.is_empty() {
            println!("{}", "No memories found.".yellow());
        } else {
            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_BOX_CHARS);
            table.set_titles(Row::new(vec![
                Cell::new("ID").style_spec("Fb"),
                Cell::new("Type").style_spec("Fb"),
                Cell::new("Content").style_spec("Fb"),
                Cell::new("Created By").style_spec("Fb"),
                Cell::new("Visibility").style_spec("Fb"),
                Cell::new("Tags").style_spec("Fb"),
            ]));

            for mem in &response.items {
                let content_preview = if mem.content.len() > 50 {
                    format!("{}…", &mem.content[..49])
                } else {
                    mem.content.clone()
                };

                table.add_row(Row::new(vec![
                    Cell::new(&mem.id),
                    Cell::new(&mem.memory_type.to_string()),
                    Cell::new(&content_preview),
                    Cell::new(&mem.created_by),
                    Cell::new(&format_visibility(&mem.visibility)),
                    Cell::new(&mem.tags.join(", ")),
                ]));
            }

            table.printstd();
        }
    }
    Ok(())
}

async fn update_visibility(
    client: &MemoryClient,
    id: &str,
    level: &str,
    share_with: Option<&str>,
    json: bool,
) -> Result<()> {
    let visibility = level
        .parse::<VisibilityLevel>()
        .map_err(|e| anyhow::anyhow!("Invalid visibility level: {e}"))?;

    let shared_vec: Option<Vec<String>> = share_with.map(|s| {
        s.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let request = UpdateVisibilityRequest {
        visibility,
        shared_with: shared_vec,
    };

    let memory = client
        .update_visibility(id, &request)
        .await
        .context(format!("Failed to update visibility for '{id}'"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!(
            "{} Visibility updated for {}",
            "✅".green(),
            id.cyan()
        );
        display_memory(&memory);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Display a full memory record with formatting.
fn display_memory(memory: &Memory) {
    println!("  {} {}", "ID:".bold(), memory.id.cyan());
    println!("  {} {}", "Type:".bold(), memory.memory_type);
    println!("  {} {}", "Content:".bold(), memory.content);
    println!("  {} {}", "Created by:".bold(), memory.created_by);
    println!(
        "  {} {}",
        "Created at:".bold(),
        memory.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "  {} {}",
        "Visibility:".bold(),
        format_visibility(&memory.visibility)
    );
    if !memory.shared_with.is_empty() {
        println!(
            "  {} {}",
            "Shared with:".bold(),
            memory.shared_with.join(", ")
        );
    }
    if !memory.tags.is_empty() {
        println!("  {} {}", "Tags:".bold(), memory.tags.join(", "));
    }
    if let Some(ref owner) = memory.owner {
        println!("  {} {}", "Owner:".bold(), owner);
    }
    if !memory.references.is_empty() {
        println!(
            "  {} {}",
            "References:".bold(),
            memory.references.join(", ")
        );
    }
}

/// Display a brief one-line-ish memory summary for search results.
fn display_memory_brief(memory: &Memory) {
    println!("  {} {}", "ID:".bold(), memory.id.cyan());
    println!(
        "  {} [{}] {}",
        "▸".dimmed(),
        memory.memory_type.to_string().yellow(),
        memory.content
    );
    if !memory.tags.is_empty() {
        println!(
            "  {} {}",
            "Tags:".dimmed(),
            memory.tags.join(", ").dimmed()
        );
    }
}

/// Format a visibility level with colour.
fn format_visibility(level: &VisibilityLevel) -> String {
    match level {
        VisibilityLevel::Public => "public".green().to_string(),
        VisibilityLevel::Shared => "shared".yellow().to_string(),
        VisibilityLevel::Private => "private".red().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_memory() -> Memory {
        let now = Utc::now();
        Memory {
            id: "mem_1_abcdef01".to_string(),
            content: "Paris is the capital of France.".to_string(),
            memory_type: MemoryType::Information,
            tags: vec!["geography".to_string(), "europe".to_string()],
            created_by: "agent-1".to_string(),
            created_at: now,
            updated_at: now,
            owner: None,
            visibility: VisibilityLevel::Public,
            shared_with: vec![],
            references: vec![],
        }
    }

    #[test]
    fn test_display_memory_does_not_panic() {
        display_memory(&sample_memory());
    }

    #[test]
    fn test_display_memory_brief_does_not_panic() {
        display_memory_brief(&sample_memory());
    }

    #[test]
    fn test_display_memory_with_all_fields() {
        let mut mem = sample_memory();
        mem.owner = Some("owner-1".to_string());
        mem.shared_with = vec!["agent-2".to_string()];
        mem.references = vec!["mem_0_ref1".to_string()];
        mem.visibility = VisibilityLevel::Shared;
        display_memory(&mem);
    }

    #[test]
    fn test_format_visibility_public() {
        let result = format_visibility(&VisibilityLevel::Public);
        assert!(result.contains("public"));
    }

    #[test]
    fn test_format_visibility_shared() {
        let result = format_visibility(&VisibilityLevel::Shared);
        assert!(result.contains("shared"));
    }

    #[test]
    fn test_format_visibility_private() {
        let result = format_visibility(&VisibilityLevel::Private);
        assert!(result.contains("private"));
    }
}
