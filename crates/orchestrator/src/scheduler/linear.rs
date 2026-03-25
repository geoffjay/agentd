//! Linear API integration: configuration and issue source.
//!
//! # Configuration
//!
//! The Linear API key can be provided in two ways, checked in this order:
//!
//! ## 1. Environment variable (recommended)
//!
//! Set `AGENTD_LINEAR_API_KEY` to your Linear personal API key:
//!
//! ```sh
//! export AGENTD_LINEAR_API_KEY=lin_api_xxxxxxxxxxxxxxxx
//! ```
//!
//! Personal API keys can be created at:
//! <https://linear.app/settings/api>
//!
//! ## 2. Config file (optional fallback)
//!
//! Add a `[linear]` section to the agentd config file:
//!
//! **Location (checked in order):**
//! - `$AGENTD_CONFIG_FILE` — explicit path override
//! - `~/.config/agentd/config.toml` (Linux / XDG)
//! - `~/Library/Application Support/agentd/config.toml` (macOS)
//!   (uses `directories::ProjectDirs::from("", "", "agentd")`)
//!
//! **Format:**
//! ```toml
//! [linear]
//! api_key = "lin_api_xxxxxxxxxxxxxxxx"
//! ```
//!
//! # Authentication
//!
//! Linear's GraphQL API endpoint is `https://api.linear.app/graphql`.
//! Authentication uses the `Authorization` header with the raw API key value —
//! no `Bearer` prefix is needed for personal API keys:
//!
//! ```text
//! Authorization: lin_api_xxxxxxxxxxxxxxxx
//! ```
//!
//! # Security
//!
//! The API key is **never** logged or included in error messages. All error
//! messages indicate only that a key is missing or present, never the key
//! value itself.

use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Linear GraphQL API endpoint.
const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// GraphQL query for fetching issues with optional filters and pagination.
const ISSUES_QUERY: &str = r#"
query Issues($filter: IssueFilter, $after: String) {
  issues(filter: $filter, after: $after, first: 50) {
    nodes {
      id
      identifier
      title
      description
      url
      state { name }
      priority
      assignee { displayName email }
      labels { nodes { name } }
      team { key name }
      project { name }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

// ---------------------------------------------------------------------------
// LinearConfig
// ---------------------------------------------------------------------------

/// Configuration for the Linear API integration.
///
/// # Security
///
/// The `api_key` field is intentionally excluded from [`std::fmt::Debug`]
/// output to prevent accidental logging. Use [`LinearConfig::is_configured`]
/// to check availability without exposing the key value.
pub struct LinearConfig {
    api_key: String,
}

/// Manually implemented to prevent the API key from appearing in log output.
impl std::fmt::Debug for LinearConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearConfig").field("api_key", &"<redacted>").finish()
    }
}

impl LinearConfig {
    /// Resolve the Linear API key from the environment or config file.
    ///
    /// Checks sources in the following order:
    ///
    /// 1. `AGENTD_LINEAR_API_KEY` environment variable
    /// 2. `[linear] api_key` in the agentd config file
    ///
    /// Returns an error if no key is found in either source. The error
    /// message **never includes** the key value.
    pub fn resolve() -> Result<Self> {
        // 1. Environment variable (preferred)
        if let Ok(key) = std::env::var("AGENTD_LINEAR_API_KEY") {
            if !key.trim().is_empty() {
                return Ok(Self { api_key: key });
            }
        }

        // 2. Config file fallback
        if let Some(key) = Self::read_from_config_file()? {
            return Ok(Self { api_key: key });
        }

        anyhow::bail!(
            "Linear API key not configured. \
             Set the AGENTD_LINEAR_API_KEY environment variable \
             or add 'api_key' to the [linear] section of the agentd config file \
             (see documentation for config file location)."
        )
    }

    /// Check whether the Linear API key is available without loading it.
    ///
    /// Returns `true` if `AGENTD_LINEAR_API_KEY` is set to a non-empty value,
    /// or if a key is present in the config file. This is a cheap check
    /// suitable for trigger validation at workflow creation time.
    pub fn is_configured() -> bool {
        if matches!(std::env::var("AGENTD_LINEAR_API_KEY"), Ok(v) if !v.trim().is_empty()) {
            return true;
        }
        match Self::read_from_config_file() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                // Config file exists but could not be read or parsed (e.g. a
                // TOML syntax error). Log a warning so the user knows why their
                // config file is being ignored rather than getting a cryptic
                // "API key not configured" message with no further context.
                warn!(
                    error = %e,
                    "Failed to read agentd config file while checking Linear API key; \
                     falling back to environment variable only"
                );
                false
            }
        }
    }

    /// Return the API key value.
    ///
    /// # Security
    ///
    /// Do **not** log, format into error messages, or include in API responses.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Attempt to read the API key from the agentd TOML config file.
    ///
    /// Returns `Ok(None)` if the config file does not exist or does not
    /// contain a `[linear] api_key` entry. Returns `Err` only if the file
    /// exists but cannot be read or parsed.
    fn read_from_config_file() -> Result<Option<String>> {
        let Some(path) = Self::config_file_path()? else {
            return Ok(None);
        };

        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read agentd config file: {}", path.display()))?;

        let value: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse agentd config file: {}", path.display()))?;

        let key = value
            .get("linear")
            .and_then(|section| section.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty());

        Ok(key)
    }

    /// Determine the path to the agentd config file.
    ///
    /// Checked in order:
    /// 1. `AGENTD_CONFIG_FILE` environment variable (explicit override)
    /// 2. Platform config directory via the `directories` crate
    ///    (`ProjectDirs::from("", "", "agentd")`):
    ///    - Linux: `$XDG_CONFIG_HOME/agentd/config.toml`
    ///      (defaults to `~/.config/agentd/config.toml`)
    ///    - macOS: `~/Library/Application Support/agentd/config.toml`
    fn config_file_path() -> Result<Option<std::path::PathBuf>> {
        // Explicit override via environment variable.
        if let Ok(p) = std::env::var("AGENTD_CONFIG_FILE") {
            return Ok(Some(std::path::PathBuf::from(p)));
        }

        // Use empty qualifier and organization so the platform path is simply
        // `<config_dir>/agentd/` on all platforms (e.g. `~/.config/agentd/`
        // on Linux, `~/Library/Application Support/agentd/` on macOS).
        // Using a non-empty organization would add an extra nesting level on
        // macOS: `~/Library/Application Support/<org>/<app>/`.
        let path = directories::ProjectDirs::from("", "", "agentd")
            .map(|d| d.config_dir().join("config.toml"));

        Ok(path)
    }
}

// ---------------------------------------------------------------------------
// GraphQL response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQLData {
    issues: IssueConnection,
}

#[derive(Debug, Deserialize)]
struct IssueConnection {
    nodes: Vec<LinearIssue>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
struct PageInfo {
    #[serde(rename = "hasNextPage")]
    has_next_page: bool,
    #[serde(rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinearIssue {
    id: String,
    identifier: String,
    title: String,
    description: Option<String>,
    url: String,
    state: Option<LinearState>,
    priority: Option<u32>,
    assignee: Option<LinearAssignee>,
    labels: LinearLabelConnection,
    team: Option<LinearTeam>,
    project: Option<LinearProject>,
}

#[derive(Debug, Deserialize)]
struct LinearState {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinearAssignee {
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct LinearLabelConnection {
    nodes: Vec<LinearLabel>,
}

#[derive(Debug, Deserialize)]
struct LinearLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinearTeam {
    key: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct LinearProject {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLError {
    message: String,
}

// ---------------------------------------------------------------------------
// LinearIssueSource
// ---------------------------------------------------------------------------

/// Fetches Linear issues via the Linear GraphQL API.
///
/// # Filters
///
/// All filter fields are optional. When multiple filters are specified they
/// are ANDed together. For labels, every listed label must be present on the
/// issue (AND semantics).
///
/// # Pagination
///
/// Iterates through all pages (50 issues per page) so that all matching
/// issues are returned in a single `fetch_tasks()` call.
///
/// # Authentication
///
/// Reads the API key via [`LinearConfig::resolve`] at construction time.
/// Construction fails fast with a clear error if no key is configured.
pub struct LinearIssueSource {
    team_key: Option<String>,
    project: Option<String>,
    status: Option<Vec<String>>,
    labels: Vec<String>,
    assignee: Option<String>,
    /// Pre-resolved API key.  Never logged.
    api_key: String,
    client: reqwest::Client,
    /// Base URL for the Linear GraphQL API.  Defaults to [`LINEAR_API_URL`].
    /// Overridable in tests to point at a local mock server.
    api_url: String,
}

/// Custom Debug excludes the API key.
impl std::fmt::Debug for LinearIssueSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearIssueSource")
            .field("team_key", &self.team_key)
            .field("project", &self.project)
            .field("status", &self.status)
            .field("labels", &self.labels)
            .field("assignee", &self.assignee)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl LinearIssueSource {
    /// Create a new source, resolving the API key immediately.
    ///
    /// Returns an error if no Linear API key is configured (see module docs
    /// for configuration options).
    pub fn new(
        team_key: Option<String>,
        project: Option<String>,
        status: Option<Vec<String>>,
        labels: Vec<String>,
        assignee: Option<String>,
    ) -> Result<Self> {
        let config = LinearConfig::resolve()?;
        Ok(Self {
            team_key,
            project,
            status,
            labels,
            assignee,
            api_key: config.api_key().to_string(),
            client: reqwest::Client::new(),
            api_url: LINEAR_API_URL.to_string(),
        })
    }

    /// Create a source with an explicit API URL.
    ///
    /// Accepts an explicit API key and base URL, bypassing [`LinearConfig::resolve`].
    /// Hidden from generated documentation (`#[doc(hidden)]`) but publicly callable —
    /// used by integration tests in `tests/linear_fetch_http.rs`.
    #[doc(hidden)]
    #[allow(dead_code)] // used by integration tests in tests/linear_fetch_http.rs
    pub fn new_with_url(
        team_key: Option<String>,
        project: Option<String>,
        status: Option<Vec<String>>,
        labels: Vec<String>,
        assignee: Option<String>,
        api_key: String,
        api_url: String,
    ) -> Self {
        Self {
            team_key,
            project,
            status,
            labels,
            assignee,
            api_key,
            client: reqwest::Client::new(),
            api_url,
        }
    }

    /// Build a Linear `IssueFilter` JSON object from the configured filters.
    ///
    /// The filter is passed as the `$filter` variable in the GraphQL query.
    ///
    /// ## Label AND semantics
    ///
    /// Linear's `IssueFilter.labels` field supports `some` (at least one
    /// label matches). To require that ALL listed labels are present, we
    /// generate one `labels.some` condition per label and combine them under
    /// the top-level `and` array alongside any other conditions.
    fn build_filter(&self) -> serde_json::Value {
        let mut filter = serde_json::Map::new();

        if let Some(key) = &self.team_key {
            filter.insert("team".to_string(), json!({ "key": { "eq": key } }));
        }

        if let Some(proj) = &self.project {
            filter.insert("project".to_string(), json!({ "name": { "containsIgnoreCase": proj } }));
        }

        if let Some(statuses) = &self.status {
            if !statuses.is_empty() {
                filter.insert("state".to_string(), json!({ "name": { "in": statuses } }));
            }
        }

        if let Some(asn) = &self.assignee {
            // Match by displayName (case-insensitive) or exact email.
            filter.insert(
                "assignee".to_string(),
                json!({
                    "or": [
                        { "displayName": { "containsIgnoreCase": asn } },
                        { "email": { "eq": asn } }
                    ]
                }),
            );
        }

        // Labels: ALL listed labels must be present (AND semantics).
        // Each `labels.some.name.eq` condition ensures one label is present;
        // combining them under `and` ensures every listed label is required.
        if !self.labels.is_empty() {
            let label_conditions: Vec<serde_json::Value> = self
                .labels
                .iter()
                .map(|l| json!({ "labels": { "some": { "name": { "eq": l } } } }))
                .collect();
            filter.insert("and".to_string(), json!(label_conditions));
        }

        serde_json::Value::Object(filter)
    }

    /// Fetch a single page of issues from the Linear API.
    async fn fetch_page(
        &self,
        filter: &serde_json::Value,
        after: Option<&str>,
    ) -> Result<IssueConnection> {
        let body = json!({
            "query": ISSUES_QUERY,
            "variables": {
                "filter": filter,
                "after": after,
            }
        });

        let response = self
            .client
            .post(self.api_url.as_str())
            // Linear personal API keys use a bare `Authorization: <key>` header —
            // no "Bearer" prefix required.
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to connect to Linear API")?;

        let status = response.status();
        if !status.is_success() {
            // Read body for a helpful error message, but never log the API key.
            let body_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Linear API returned HTTP {}: {}", status, body_text.trim());
        }

        let gql: GraphQLResponse =
            response.json().await.context("Failed to parse Linear API response")?;

        // Surface any GraphQL-level errors.
        if let Some(errors) = gql.errors {
            let msgs: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            anyhow::bail!("Linear API returned errors: {}", msgs.join("; "));
        }

        let data = gql.data.context("Linear API response contained no data")?;
        Ok(data.issues)
    }
}

#[async_trait]
impl TaskSource for LinearIssueSource {
    /// Fetch all matching issues from Linear, paginating through all pages.
    async fn fetch_tasks(&self) -> Result<Vec<Task>> {
        let filter = self.build_filter();
        let mut all_issues: Vec<LinearIssue> = Vec::new();
        let mut cursor: Option<String> = None;

        debug!(
            team_key = ?self.team_key,
            project = ?self.project,
            "Fetching Linear issues"
        );

        loop {
            let page =
                self.fetch_page(&filter, cursor.as_deref()).await.context("Linear page fetch")?;

            let fetched = page.nodes.len();
            all_issues.extend(page.nodes);

            debug!(fetched, total = all_issues.len(), "Fetched page of Linear issues");

            if !page.page_info.has_next_page || page.page_info.end_cursor.is_none() {
                break;
            }
            cursor = page.page_info.end_cursor;
        }

        Ok(map_issues(all_issues))
    }

    fn source_type(&self) -> &'static str {
        "linear_issues"
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

/// Map a list of raw Linear issues to [`Task`] structs.
fn map_issues(issues: Vec<LinearIssue>) -> Vec<Task> {
    issues.into_iter().map(map_issue).collect()
}

fn map_issue(issue: LinearIssue) -> Task {
    let mut metadata = HashMap::new();
    // Store the Linear internal UUID so downstream code (e.g. webhooks) can
    // look up issues by their stable ID without re-parsing the identifier.
    metadata.insert("linear_id".to_string(), issue.id.clone());
    metadata.insert("identifier".to_string(), issue.identifier.clone());
    if let Some(state) = &issue.state {
        metadata.insert("state".to_string(), state.name.clone());
    }
    if let Some(priority) = issue.priority {
        metadata.insert("priority".to_string(), priority.to_string());
    }
    if let Some(team) = &issue.team {
        metadata.insert("team".to_string(), team.key.clone());
        metadata.insert("team_name".to_string(), team.name.clone());
    }
    if let Some(project) = &issue.project {
        metadata.insert("project".to_string(), project.name.clone());
    }

    Task {
        source_id: issue.identifier,
        title: issue.title,
        body: issue.description.unwrap_or_default(),
        url: issue.url,
        labels: issue.labels.nodes.into_iter().map(|l| l.name).collect(),
        assignee: issue.assignee.map(|a| a.display_name),
        metadata,
    }
}

/// Parse a JSON array of Linear issue nodes into [`Task`] structs.
///
/// The input is the `nodes` array from a Linear GraphQL `issues` query
/// response — the same structure used in `fetch_tasks()` internally.
///
/// This helper is provided for testing and offline use, following the
/// pattern established by `parse_gh_issues()` in `github.rs`.
///
/// # Example
///
/// ```rust,ignore
/// let json = r#"[{"id":"abc","identifier":"ENG-1","title":"Fix it",...}]"#;
/// let tasks = parse_linear_issues(json)?;
/// ```
#[allow(dead_code)]
pub fn parse_linear_issues(json: &str) -> Result<Vec<Task>> {
    let issues: Vec<LinearIssue> = serde_json::from_str(json)?;
    Ok(map_issues(issues))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Helpers shared between LinearConfig and LinearIssueSource tests
    // -------------------------------------------------------------------------

    /// Global mutex to serialize env-var-mutating tests.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_env_lock(f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        f();
    }

    struct EnvRestorer {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestorer {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestorer {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => std::env::set_var(self.key, val),
                None => std::env::remove_var(self.key),
            }
        }
    }

    // -------------------------------------------------------------------------
    // LinearConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn is_configured_returns_false_when_env_unset() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let tmp = std::env::temp_dir().join("agentd_linear_test_nonexistent.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());
            assert!(!LinearConfig::is_configured());
        });
    }

    #[test]
    fn is_configured_returns_true_when_env_set() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_testkey");
            assert!(LinearConfig::is_configured());
        });
    }

    #[test]
    fn resolve_succeeds_when_env_set() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_testkey123");
            let cfg = LinearConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.api_key(), "lin_api_testkey123");
        });
    }

    #[test]
    fn resolve_fails_when_no_key_available() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let tmp = std::env::temp_dir().join("agentd_linear_test_nonexistent2.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());

            let err = LinearConfig::resolve().expect_err("should fail without key");
            let msg = err.to_string();
            assert!(msg.contains("AGENTD_LINEAR_API_KEY"), "message: {msg}");
            assert!(!msg.contains("lin_api_"), "key must not appear in error: {msg}");
        });
    }

    #[test]
    fn resolve_reads_key_from_config_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[linear]\napi_key = \"lin_api_fromfile\"\n")
                .expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let cfg = LinearConfig::resolve().expect("should resolve from file");
            assert_eq!(cfg.api_key(), "lin_api_fromfile");
        });
    }

    #[test]
    fn resolve_env_takes_precedence_over_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_fromenv");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[linear]\napi_key = \"lin_api_fromfile\"\n")
                .expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let cfg = LinearConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.api_key(), "lin_api_fromenv");
        });
    }

    #[test]
    fn config_file_missing_linear_section_returns_none() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[github]\ntoken = \"gh_token\"\n").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let err = LinearConfig::resolve().expect_err("should fail — no linear key");
            assert!(err.to_string().contains("AGENTD_LINEAR_API_KEY"));
        });
    }

    #[test]
    fn is_configured_returns_false_for_malformed_config_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "this is [not valid toml syntax !!!").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            assert!(!LinearConfig::is_configured());
            let err = LinearConfig::resolve().expect_err("should fail — malformed config");
            let msg = err.to_string();
            assert!(msg.contains("parse") || msg.contains("config file"), "message: {msg}");
        });
    }

    // -------------------------------------------------------------------------
    // parse_linear_issues tests
    // -------------------------------------------------------------------------

    fn full_issue_json() -> &'static str {
        r#"[
            {
                "id": "abc123",
                "identifier": "ENG-42",
                "title": "Fix the widget",
                "description": "The widget is broken and needs fixing.",
                "url": "https://linear.app/team/issue/ENG-42",
                "state": { "name": "In Progress" },
                "priority": 2,
                "assignee": { "displayName": "Alice Smith", "email": "alice@example.com" },
                "labels": { "nodes": [{"name": "bug"}, {"name": "urgent"}] },
                "team": { "key": "ENG", "name": "Engineering" },
                "project": { "name": "Q1 Roadmap" }
            },
            {
                "id": "def456",
                "identifier": "ENG-43",
                "title": "Add feature X",
                "description": null,
                "url": "https://linear.app/team/issue/ENG-43",
                "state": { "name": "Todo" },
                "priority": null,
                "assignee": null,
                "labels": { "nodes": [] },
                "team": { "key": "ENG", "name": "Engineering" },
                "project": null
            }
        ]"#
    }

    #[test]
    fn test_parse_linear_issues_full() {
        let tasks = parse_linear_issues(full_issue_json()).unwrap();
        assert_eq!(tasks.len(), 2);

        let t0 = &tasks[0];
        assert_eq!(t0.source_id, "ENG-42");
        assert_eq!(t0.title, "Fix the widget");
        assert_eq!(t0.body, "The widget is broken and needs fixing.");
        assert_eq!(t0.url, "https://linear.app/team/issue/ENG-42");
        assert_eq!(t0.labels, vec!["bug", "urgent"]);
        assert_eq!(t0.assignee, Some("Alice Smith".to_string()));
        assert_eq!(t0.metadata.get("identifier").unwrap(), "ENG-42");
        assert_eq!(t0.metadata.get("state").unwrap(), "In Progress");
        assert_eq!(t0.metadata.get("priority").unwrap(), "2");
        assert_eq!(t0.metadata.get("team").unwrap(), "ENG");
        assert_eq!(t0.metadata.get("team_name").unwrap(), "Engineering");
        assert_eq!(t0.metadata.get("project").unwrap(), "Q1 Roadmap");
    }

    #[test]
    fn test_parse_linear_issues_minimal() {
        let tasks = parse_linear_issues(full_issue_json()).unwrap();
        let t1 = &tasks[1];
        assert_eq!(t1.source_id, "ENG-43");
        assert_eq!(t1.body, "");
        assert!(t1.assignee.is_none());
        assert!(t1.labels.is_empty());
        assert!(!t1.metadata.contains_key("priority"));
        assert!(!t1.metadata.contains_key("project"));
        assert_eq!(t1.metadata.get("state").unwrap(), "Todo");
    }

    #[test]
    fn test_parse_linear_issues_empty() {
        let tasks = parse_linear_issues("[]").unwrap();
        assert!(tasks.is_empty());
    }

    // -------------------------------------------------------------------------
    // build_filter tests
    // -------------------------------------------------------------------------

    fn make_source(
        team_key: Option<&str>,
        project: Option<&str>,
        status: Option<Vec<&str>>,
        labels: Vec<&str>,
        assignee: Option<&str>,
    ) -> LinearIssueSource {
        LinearIssueSource {
            team_key: team_key.map(str::to_string),
            project: project.map(str::to_string),
            status: status.map(|v| v.into_iter().map(str::to_string).collect()),
            labels: labels.into_iter().map(str::to_string).collect(),
            assignee: assignee.map(str::to_string),
            api_key: "test_key".to_string(),
            client: reqwest::Client::new(),
            api_url: LINEAR_API_URL.to_string(),
        }
    }

    #[test]
    fn build_filter_empty() {
        let src = make_source(None, None, None, vec![], None);
        let filter = src.build_filter();
        // Empty filter — no fields set.
        assert!(filter.as_object().unwrap().is_empty());
    }

    #[test]
    fn build_filter_team_key() {
        let src = make_source(Some("ENG"), None, None, vec![], None);
        let filter = src.build_filter();
        assert_eq!(filter["team"]["key"]["eq"], "ENG");
    }

    #[test]
    fn build_filter_status() {
        let src = make_source(None, None, Some(vec!["Todo", "In Progress"]), vec![], None);
        let filter = src.build_filter();
        let statuses = filter["state"]["name"]["in"].as_array().unwrap();
        assert_eq!(statuses.len(), 2);
        assert!(statuses.iter().any(|v| v == "Todo"));
        assert!(statuses.iter().any(|v| v == "In Progress"));
    }

    #[test]
    fn build_filter_single_label() {
        let src = make_source(None, None, None, vec!["bug"], None);
        let filter = src.build_filter();
        let and = filter["and"].as_array().unwrap();
        assert_eq!(and.len(), 1);
        assert_eq!(and[0]["labels"]["some"]["name"]["eq"], "bug");
    }

    #[test]
    fn build_filter_multiple_labels_and_semantics() {
        let src = make_source(None, None, None, vec!["bug", "urgent"], None);
        let filter = src.build_filter();
        let and = filter["and"].as_array().unwrap();
        assert_eq!(and.len(), 2);
        let names: Vec<&str> =
            and.iter().map(|c| c["labels"]["some"]["name"]["eq"].as_str().unwrap()).collect();
        assert!(names.contains(&"bug"));
        assert!(names.contains(&"urgent"));
    }

    #[test]
    fn build_filter_assignee() {
        let src = make_source(None, None, None, vec![], Some("alice@example.com"));
        let filter = src.build_filter();
        let or = filter["assignee"]["or"].as_array().unwrap();
        assert_eq!(or.len(), 2);
        // One branch matches by email, other by displayName.
        let has_email = or.iter().any(|c| c["email"]["eq"] == "alice@example.com");
        let has_display = or.iter().any(|c| !c["displayName"].is_null());
        assert!(has_email);
        assert!(has_display);
    }

    #[test]
    fn build_filter_combined() {
        let src = make_source(
            Some("ENG"),
            Some("Roadmap"),
            Some(vec!["Todo"]),
            vec!["bug"],
            Some("alice"),
        );
        let filter = src.build_filter();
        assert!(filter["team"]["key"]["eq"] == "ENG");
        assert!(!filter["project"].is_null());
        assert!(!filter["state"].is_null());
        assert!(!filter["assignee"].is_null());
        let and = filter["and"].as_array().unwrap();
        assert_eq!(and.len(), 1); // one label
    }

    #[test]
    fn source_type_is_linear_issues() {
        let src = make_source(None, None, None, vec![], None);
        assert_eq!(src.source_type(), "linear_issues");
    }

    #[test]
    fn debug_does_not_expose_api_key() {
        let src = make_source(None, None, None, vec![], None);
        let debug_str = format!("{src:?}");
        assert!(debug_str.contains("<redacted>"));
        assert!(!debug_str.contains("test_key"));
    }
}
