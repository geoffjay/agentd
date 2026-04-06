//! Index service CLI subcommands.
//!
//! Provides `agent index` subcommands for interacting with the agentd-index
//! service (default port 17012).
//!
//! # Available Commands
//!
//! | Command       | Description                                   |
//! |---------------|-----------------------------------------------|
//! | `health`      | Check the index service health                |
//! | `add-repo`    | Register a repository for indexing            |
//! | `remove-repo` | Remove a registered repository                |
//! | `list-repos`  | List all registered repositories              |
//! | `status`      | Show the indexing status of a repository      |
//! | `reindex`     | Trigger a re-index for a repository           |
//! | `search`      | Search indexed code with semantic / hybrid    |
//!
//! # Examples
//!
//! ```bash
//! # Register a repository
//! agent index add-repo --name agentd --path /home/user/agentd
//!
//! # List all repositories
//! agent index list-repos
//!
//! # Check indexing status
//! agent index status <repo-id>
//!
//! # Trigger re-index
//! agent index reindex <repo-id>
//!
//! # Search code
//! agent index search "authentication middleware" --limit 5
//! agent index search "async error handling" --mode hybrid --language rust
//! ```

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::*;
use prettytable::{format, Cell, Row, Table};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---------------------------------------------------------------------------
// IndexClient
// ---------------------------------------------------------------------------

/// Thin HTTP client for the agentd-index service.
pub struct IndexClient {
    base_url: String,
    http: reqwest::Client,
}

impl IndexClient {
    /// Create a new client connecting to `base_url`.
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self { base_url: base_url.into(), http }
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.http.get(&url).send().await.with_context(|| format!("GET {url}"))
    }

    async fn post_json<T: Serialize>(&self, path: &str, body: &T) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.http.post(&url).json(body).send().await.with_context(|| format!("POST {url}"))
    }

    async fn post_empty(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.http.post(&url).send().await.with_context(|| format!("POST {url}"))
    }

    async fn delete(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.http.delete(&url).send().await.with_context(|| format!("DELETE {url}"))
    }
}

// ---------------------------------------------------------------------------
// Response types (subset — only the fields we need to display)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RepoRecord {
    id: String,
    name: String,
    path: String,
    status: String,
    // Deserialized for completeness; not all fields are displayed.
    #[allow(dead_code)]
    created_at: String,
    #[allow(dead_code)]
    updated_at: String,
    last_indexed: Option<String>,
    #[allow(dead_code)]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoListResponse {
    repositories: Vec<RepoRecord>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct RepoStatusResponse {
    id: String,
    status: String,
    last_indexed: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    id: String,
    file_path: String,
    language: String,
    chunk_type: String,
    symbol_name: Option<String>,
    start_line: usize,
    end_line: usize,
    content: String,
    score: f32,
    repo_id: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    total: usize,
    query_time_ms: u64,
}

#[derive(Debug, Serialize)]
struct SearchRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hierarchy_level: Option<String>,
    limit: usize,
    search_mode: String,
}

#[derive(Debug, Serialize)]
struct AddRepoRequest {
    name: String,
    path: String,
}

// ---------------------------------------------------------------------------
// IndexCommand
// ---------------------------------------------------------------------------

/// Subcommands for the agentd-index service.
#[derive(Debug, Subcommand)]
pub enum IndexCommand {
    /// Check the health of the index service.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index health
    /// ```
    Health,

    /// Register a repository for indexing.
    ///
    /// Adds the repository to the index service registry.  The service will
    /// begin indexing it in the background after registration.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index add-repo --name agentd --path /home/user/agentd
    /// agent index add-repo --name myproject --path /projects/myproject --json
    /// ```
    AddRepo {
        /// Human-readable name for the repository.
        #[arg(long, short)]
        name: String,

        /// Absolute path to the repository root on the local filesystem.
        #[arg(long, short)]
        path: String,
    },

    /// Remove a registered repository.
    ///
    /// Deletes the repository record from the index service.  Indexed chunks
    /// for this repository are NOT automatically removed from the vector store.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index remove-repo <repo-id>
    /// ```
    RemoveRepo {
        /// Repository ID to remove.
        id: String,
    },

    /// List all registered repositories.
    ///
    /// Shows each repository's name, path, status, and last indexed time.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index list-repos
    /// agent index list-repos --json
    /// ```
    ListRepos,

    /// Show the indexing status of a repository.
    ///
    /// Returns the current status (pending / indexing / ready / error) and
    /// the timestamp of the last successful index run.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index status <repo-id>
    /// ```
    Status {
        /// Repository ID.
        id: String,
    },

    /// Trigger a full re-index for a repository.
    ///
    /// Marks the repository as pending so the background watcher will pick
    /// it up and run a new index pass.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index reindex <repo-id>
    /// ```
    Reindex {
        /// Repository ID to re-index.
        id: String,
    },

    /// Search indexed code with semantic or hybrid search.
    ///
    /// Queries the vector index and returns ranked code chunks that match
    /// the natural-language query.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent index search "authentication middleware"
    /// agent index search "async error handling" --mode hybrid --language rust
    /// agent index search "database connection pool" --repo agentd --limit 5
    /// ```
    Search {
        /// Natural-language query or identifier to search for.
        query: String,

        /// Search mode: `vector` (default), `keyword`, or `hybrid`.
        #[arg(long, default_value = "vector")]
        mode: String,

        /// Filter results by repository ID.
        #[arg(long)]
        repo: Option<String>,

        /// Filter results by programming language (e.g. `rust`, `python`).
        #[arg(long)]
        language: Option<String>,

        /// Filter results by file glob pattern (e.g. `src/auth/**`).
        #[arg(long)]
        file_pattern: Option<String>,

        /// Filter by hierarchy level: `symbol`, `file`, `directory`, `repository`.
        #[arg(long)]
        hierarchy: Option<String>,

        /// Maximum number of results to return (default: 10).
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

impl IndexCommand {
    /// Execute the command against the given index service client.
    pub async fn execute(&self, client: &IndexClient, json: bool) -> Result<()> {
        match self {
            IndexCommand::Health => health(client, json).await,
            IndexCommand::AddRepo { name, path } => add_repo(client, name, path, json).await,
            IndexCommand::RemoveRepo { id } => remove_repo(client, id).await,
            IndexCommand::ListRepos => list_repos(client, json).await,
            IndexCommand::Status { id } => status(client, id, json).await,
            IndexCommand::Reindex { id } => reindex(client, id, json).await,
            IndexCommand::Search {
                query,
                mode,
                repo,
                language,
                file_pattern,
                hierarchy,
                limit,
            } => {
                search(client, query, mode, repo, language, file_pattern, hierarchy, *limit, json)
                    .await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handler functions
// ---------------------------------------------------------------------------

async fn health(client: &IndexClient, json: bool) -> Result<()> {
    let resp = client.get("/health").await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.context("parse health response")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(());
    }

    if status.is_success() {
        println!("{} agentd-index is {}", "✅".green(), "healthy".green().bold());
        if let Some(version) = body.get("version").and_then(|v| v.as_str()) {
            println!("   Version: {}", version.cyan());
        }
    } else {
        println!("{} agentd-index is {}", "❌".red(), "unhealthy".red().bold());
        println!("   HTTP {status}");
    }
    Ok(())
}

async fn add_repo(client: &IndexClient, name: &str, path: &str, json: bool) -> Result<()> {
    let body = AddRepoRequest { name: name.to_string(), path: path.to_string() };
    let resp = client.post_json("/repositories", &body).await?;
    let status = resp.status();

    if !status.is_success() {
        let err: serde_json::Value = resp.json().await.unwrap_or_default();
        bail!(
            "Failed to register repository (HTTP {}): {}",
            status,
            err["error"].as_str().unwrap_or("unknown")
        );
    }

    let record: RepoRecord = resp.json().await.context("parse create-repo response")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "id": record.id,
                "name": record.name,
                "path": record.path,
                "status": record.status,
            }))?
        );
        return Ok(());
    }

    println!("{} Repository registered", "✅".green());
    println!("   ID:     {}", record.id.cyan());
    println!("   Name:   {}", record.name.bold());
    println!("   Path:   {}", record.path);
    println!("   Status: {}", format_status(&record.status));
    Ok(())
}

async fn remove_repo(client: &IndexClient, id: &str) -> Result<()> {
    let resp = client.delete(&format!("/repositories/{id}")).await?;
    let status = resp.status();

    if status == reqwest::StatusCode::NO_CONTENT {
        println!("{} Repository {} removed", "✅".green(), id.cyan());
        return Ok(());
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        bail!("Repository not found: {id}");
    }
    let err: serde_json::Value = resp.json().await.unwrap_or_default();
    bail!(
        "Failed to remove repository (HTTP {}): {}",
        status,
        err["error"].as_str().unwrap_or("unknown")
    );
}

async fn list_repos(client: &IndexClient, json: bool) -> Result<()> {
    let resp = client.get("/repositories").await?;
    if !resp.status().is_success() {
        bail!("Index service returned HTTP {}", resp.status());
    }
    let data: RepoListResponse = resp.json().await.context("parse list-repos response")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "repositories": data.repositories.iter().map(|r| serde_json::json!({
                    "id": r.id, "name": r.name, "path": r.path,
                    "status": r.status, "last_indexed": r.last_indexed,
                })).collect::<Vec<_>>(),
                "total": data.total,
            }))?
        );
        return Ok(());
    }

    if data.repositories.is_empty() {
        println!("{}", "No repositories registered.".bright_black());
        println!("  Run: agent index add-repo --name <name> --path <path>");
        return Ok(());
    }

    println!("{}", format!("Registered repositories ({})", data.total).bold());
    println!("{}", "─".repeat(70).bright_black());

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(Row::new(vec![
        Cell::new("NAME").style_spec("Fb"),
        Cell::new("STATUS").style_spec("Fb"),
        Cell::new("PATH").style_spec("Fb"),
        Cell::new("LAST INDEXED").style_spec("Fb"),
        Cell::new("ID").style_spec("Fb"),
    ]));

    for repo in &data.repositories {
        let last = repo.last_indexed.as_deref().unwrap_or("-");
        table.add_row(Row::new(vec![
            Cell::new(&repo.name),
            Cell::new(&repo.status),
            Cell::new(&repo.path),
            Cell::new(last),
            Cell::new(&repo.id[..8.min(repo.id.len())]),
        ]));
    }
    table.printstd();
    Ok(())
}

async fn status(client: &IndexClient, id: &str, json: bool) -> Result<()> {
    let resp = client.get(&format!("/repositories/{id}/status")).await?;
    let http_status = resp.status();

    if http_status == reqwest::StatusCode::NOT_FOUND {
        bail!("Repository not found: {id}");
    }
    if !http_status.is_success() {
        bail!("Index service returned HTTP {http_status}");
    }

    let data: RepoStatusResponse = resp.json().await.context("parse status response")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "id": data.id,
                "status": data.status,
                "last_indexed": data.last_indexed,
                "error_message": data.error_message,
            }))?
        );
        return Ok(());
    }

    println!("Repository: {}", id.cyan());
    println!("Status:     {}", format_status(&data.status));
    if let Some(ts) = &data.last_indexed {
        println!("Indexed:    {}", ts.bright_black());
    }
    if let Some(err) = &data.error_message {
        println!("Error:      {}", err.red());
    }
    Ok(())
}

async fn reindex(client: &IndexClient, id: &str, json: bool) -> Result<()> {
    let resp = client.post_empty(&format!("/repositories/{id}/reindex")).await?;
    let http_status = resp.status();

    if http_status == reqwest::StatusCode::NOT_FOUND {
        bail!("Repository not found: {id}");
    }
    if !http_status.is_success() {
        let err: serde_json::Value = resp.json().await.unwrap_or_default();
        bail!(
            "Failed to trigger reindex (HTTP {}): {}",
            http_status,
            err["error"].as_str().unwrap_or("unknown")
        );
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "status": "pending" }))?);
        return Ok(());
    }

    println!("{} Re-index triggered for repository {}", "✅".green(), id.cyan());
    println!("   Status: {}", "pending".yellow());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn search(
    client: &IndexClient,
    query: &str,
    mode: &str,
    repo: &Option<String>,
    language: &Option<String>,
    file_pattern: &Option<String>,
    hierarchy: &Option<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let request = SearchRequest {
        query: query.to_string(),
        repo_id: repo.clone(),
        language: language.clone(),
        file_pattern: file_pattern.clone(),
        hierarchy_level: hierarchy.clone(),
        limit,
        search_mode: mode.to_string(),
    };

    let resp = client.post_json("/search", &request).await?;
    let http_status = resp.status();

    if !http_status.is_success() {
        let err: serde_json::Value = resp.json().await.unwrap_or_default();
        bail!(
            "Search failed (HTTP {}): {}",
            http_status,
            err["error"].as_str().unwrap_or("unknown")
        );
    }

    let data: SearchResponse = resp.json().await.context("parse search response")?;

    if json {
        let output: Vec<serde_json::Value> = data
            .results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "file_path": r.file_path,
                    "language": r.language,
                    "chunk_type": r.chunk_type,
                    "symbol_name": r.symbol_name,
                    "start_line": r.start_line,
                    "end_line": r.end_line,
                    "score": r.score,
                    "repo_id": r.repo_id,
                    "content": r.content,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "results": output,
                "total": data.total,
                "query_time_ms": data.query_time_ms,
            }))?
        );
        return Ok(());
    }

    println!(
        "{} {} result(s) in {}ms",
        "🔍".cyan(),
        data.total.to_string().bold(),
        data.query_time_ms
    );

    if data.results.is_empty() {
        println!("{}", "No results found.".bright_black());
        return Ok(());
    }

    println!("{}", "─".repeat(70).bright_black());

    for (i, result) in data.results.iter().enumerate() {
        let score_pct = (result.score * 100.0) as u32;
        let symbol = result.symbol_name.as_deref().unwrap_or("<anonymous>");

        println!(
            "{}. {} {} {}",
            (i + 1).to_string().bright_black(),
            symbol.bold().cyan(),
            format!("({})", result.language).bright_black(),
            format!("[{score_pct}%]").green(),
        );
        println!(
            "   {} {}:{}–{}",
            "📄".bright_black(),
            result.file_path.bright_black(),
            result.start_line,
            result.end_line,
        );

        // Print a snippet (first 3 lines of content).
        let snippet: String = result
            .content
            .lines()
            .take(3)
            .map(|l| format!("   {}", l))
            .collect::<Vec<_>>()
            .join("\n");
        println!("{}", snippet.bright_black());

        if i + 1 < data.results.len() {
            println!();
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a repo status string with colour.
fn format_status(status: &str) -> colored::ColoredString {
    match status {
        "ready" => status.green().bold(),
        "indexing" => status.yellow().bold(),
        "pending" => status.cyan(),
        "error" => status.red().bold(),
        other => other.normal(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_status_ready() {
        // Just ensure it doesn't panic and returns something.
        let _ = format_status("ready");
        let _ = format_status("indexing");
        let _ = format_status("pending");
        let _ = format_status("error");
        let _ = format_status("unknown");
    }

    #[test]
    fn search_request_serializes_correctly() {
        let req = SearchRequest {
            query: "auth handler".to_string(),
            repo_id: Some("repo1".to_string()),
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: 10,
            search_mode: "vector".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("auth handler"));
        assert!(json.contains("vector"));
        assert!(json.contains("repo1"));
        // None fields should be omitted.
        assert!(!json.contains("language"));
    }

    #[test]
    fn search_request_omits_none_fields() {
        let req = SearchRequest {
            query: "test".to_string(),
            repo_id: None,
            language: None,
            file_pattern: None,
            hierarchy_level: None,
            limit: 5,
            search_mode: "hybrid".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("repo_id"));
        assert!(!json.contains("language"));
        assert!(!json.contains("file_pattern"));
    }
}
