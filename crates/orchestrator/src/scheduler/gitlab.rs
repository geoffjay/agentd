//! GitLab API integration: configuration, issue source, and merge request source.
//!
//! # Configuration
//!
//! The GitLab API token can be provided in two ways, checked in this order:
//!
//! ## 1. Environment variable (recommended)
//!
//! ```sh
//! export AGENTD_GITLAB_TOKEN=glpat-xxxxxxxxxxxxxxxxxxxx
//! ```
//!
//! ## 2. Config file (optional fallback)
//!
//! Add a `[gitlab]` section to the agentd config file:
//!
//! **Location (checked in order):**
//! - `$AGENTD_CONFIG_FILE` — explicit path override
//! - `~/.config/agentd/config.toml` (Linux / XDG)
//! - `~/Library/Application Support/agentd/config.toml` (macOS)
//!
//! **Format:**
//! ```toml
//! [gitlab]
//! token = "glpat-xxxxxxxxxxxxxxxxxxxx"
//! ```
//!
//! ## Self-hosted GitLab
//!
//! Set `AGENTD_GITLAB_URL` to override the default `https://gitlab.com` base URL:
//!
//! ```sh
//! export AGENTD_GITLAB_URL=https://gitlab.example.com
//! ```
//!
//! # Authentication
//!
//! GitLab REST API v4 uses the `PRIVATE-TOKEN` header:
//!
//! ```text
//! PRIVATE-TOKEN: glpat-xxxxxxxxxxxxxxxxxxxx
//! ```
//!
//! # Security
//!
//! The token is **never** logged or included in error messages.

use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Default GitLab base URL (gitlab.com SaaS instance).
const DEFAULT_GITLAB_URL: &str = "https://gitlab.com";

// ---------------------------------------------------------------------------
// GitlabConfig
// ---------------------------------------------------------------------------

/// Configuration for the GitLab REST API integration.
///
/// # Security
///
/// The `token` field is intentionally excluded from [`std::fmt::Debug`] output
/// to prevent accidental logging. Use [`GitlabConfig::is_configured`] to check
/// availability without exposing the token value.
pub struct GitlabConfig {
    token: String,
    base_url: String,
}

/// Manually implemented to prevent the token from appearing in log output.
impl std::fmt::Debug for GitlabConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitlabConfig")
            .field("token", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl GitlabConfig {
    /// Resolve the GitLab token from the environment or config file.
    ///
    /// Checks sources in the following order:
    ///
    /// 1. `AGENTD_GITLAB_TOKEN` environment variable
    /// 2. `[gitlab] token` in the agentd config file
    ///
    /// The base URL is read from `AGENTD_GITLAB_URL` (defaults to
    /// `https://gitlab.com`).
    ///
    /// Returns an error if no token is found. The error message **never
    /// includes** the token value.
    pub fn resolve() -> Result<Self> {
        let base_url =
            std::env::var("AGENTD_GITLAB_URL").unwrap_or_else(|_| DEFAULT_GITLAB_URL.to_string());

        // 1. Environment variable (preferred)
        if let Ok(token) = std::env::var("AGENTD_GITLAB_TOKEN") {
            if !token.trim().is_empty() {
                return Ok(Self { token, base_url });
            }
        }

        // 2. Config file fallback
        if let Some(token) = Self::read_from_config_file()? {
            return Ok(Self { token, base_url });
        }

        anyhow::bail!(
            "GitLab token not configured. \
             Set the AGENTD_GITLAB_TOKEN environment variable \
             or add 'token' to the [gitlab] section of the agentd config file."
        )
    }

    /// Check whether a GitLab token is available without loading it.
    ///
    /// Returns `true` if `AGENTD_GITLAB_TOKEN` is set to a non-empty value,
    /// or if a token is present in the config file.
    pub fn is_configured() -> bool {
        if matches!(std::env::var("AGENTD_GITLAB_TOKEN"), Ok(v) if !v.trim().is_empty()) {
            return true;
        }
        match Self::read_from_config_file() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                warn!(
                    error = %e,
                    "Failed to read agentd config file while checking GitLab token; \
                     falling back to environment variable only"
                );
                false
            }
        }
    }

    /// Return the token value.
    ///
    /// # Security
    ///
    /// Do **not** log, format into error messages, or include in API responses.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Return the base URL (e.g. `https://gitlab.com` or a self-hosted URL).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Attempt to read the token from the agentd TOML config file.
    ///
    /// Returns `Ok(None)` if the config file does not exist or does not
    /// contain a `[gitlab] token` entry.
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

        let token = value
            .get("gitlab")
            .and_then(|section| section.get("token"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty());

        Ok(token)
    }

    /// Determine the path to the agentd config file.
    fn config_file_path() -> Result<Option<std::path::PathBuf>> {
        if let Ok(p) = std::env::var("AGENTD_CONFIG_FILE") {
            return Ok(Some(std::path::PathBuf::from(p)));
        }

        let path = directories::ProjectDirs::from("", "", "agentd")
            .map(|d| d.config_dir().join("config.toml"));

        Ok(path)
    }
}

// ---------------------------------------------------------------------------
// GitLab REST API response types
// ---------------------------------------------------------------------------

/// A GitLab user stub used in assignee lists.
#[derive(Debug, Deserialize)]
struct GitlabUser {
    username: String,
}

/// A single GitLab issue as returned by `GET /api/v4/projects/:id/issues`.
#[derive(Debug, Deserialize)]
struct GitlabIssue {
    iid: u64,
    title: String,
    description: Option<String>,
    web_url: String,
    state: String,
    labels: Vec<String>,
    assignees: Vec<GitlabUser>,
    project_id: u64,
}

/// A single GitLab merge request as returned by
/// `GET /api/v4/projects/:id/merge_requests`.
#[derive(Debug, Deserialize)]
struct GitlabMergeRequest {
    iid: u64,
    title: String,
    description: Option<String>,
    web_url: String,
    state: String,
    labels: Vec<String>,
    assignees: Vec<GitlabUser>,
    source_branch: String,
    target_branch: String,
    merge_status: Option<String>,
    draft: bool,
    project_id: u64,
}

// ---------------------------------------------------------------------------
// GitlabIssueSource
// ---------------------------------------------------------------------------

/// Fetches GitLab issues via the REST API v4.
///
/// # Filters
///
/// - `state` — GitLab uses `opened`/`closed`/`all` (note: **`opened`**, not `open`)
/// - `labels` — comma-separated label names
/// - `assignee` — filter by `assignee_username`
///
/// # Pagination
///
/// Iterates through all pages (`per_page=100`) until no more results.
pub struct GitlabIssueSource {
    owner: String,
    repo: String,
    labels: Vec<String>,
    state: String,
    assignee: Option<String>,
    /// Pre-resolved token. Never logged.
    token: String,
    /// GitLab base URL (e.g. `https://gitlab.com` or self-hosted).
    base_url: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for GitlabIssueSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitlabIssueSource")
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .field("labels", &self.labels)
            .field("state", &self.state)
            .field("assignee", &self.assignee)
            .field("token", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl GitlabIssueSource {
    /// Create a new source, resolving the GitLab token immediately.
    pub fn new(
        owner: String,
        repo: String,
        labels: Vec<String>,
        state: String,
        assignee: Option<String>,
    ) -> Result<Self> {
        let config = GitlabConfig::resolve()?;
        Ok(Self {
            owner,
            repo,
            labels,
            state,
            assignee,
            token: config.token().to_string(),
            base_url: config.base_url().to_string(),
            client: reqwest::Client::new(),
        })
    }

    /// Create a source with an explicit token and base URL (for testing).
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn new_with_config(
        owner: String,
        repo: String,
        labels: Vec<String>,
        state: String,
        assignee: Option<String>,
        token: String,
        base_url: String,
    ) -> Self {
        Self {
            owner,
            repo,
            labels,
            state,
            assignee,
            token,
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch a single page of issues.
    async fn fetch_page(&self, page: u32) -> Result<Vec<GitlabIssue>> {
        // GitLab uses `owner%2Frepo` as the project identifier.
        let project_id = format!("{}/{}", self.owner, self.repo);
        let encoded = urlencoding::encode(&project_id);
        let url = format!("{}/api/v4/projects/{}/issues", self.base_url, encoded);

        let mut request = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .query(&[("state", self.state.as_str()), ("per_page", "100")])
            .query(&[("page", page)]);

        if !self.labels.is_empty() {
            request = request.query(&[("labels", self.labels.join(","))]);
        }
        if let Some(assignee) = &self.assignee {
            request = request.query(&[("assignee_username", assignee.as_str())]);
        }

        let response = request.send().await.context("Failed to connect to GitLab API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitLab API returned HTTP {}: {}", status, body.trim());
        }

        let issues: Vec<GitlabIssue> =
            response.json().await.context("Failed to parse GitLab issues response")?;
        Ok(issues)
    }
}

#[async_trait]
impl TaskSource for GitlabIssueSource {
    async fn fetch_tasks(&self) -> Result<Vec<Task>> {
        debug!(
            owner = %self.owner,
            repo = %self.repo,
            state = %self.state,
            "Fetching GitLab issues"
        );

        let mut all_issues: Vec<GitlabIssue> = Vec::new();
        let mut page = 1u32;

        loop {
            let page_issues = self
                .fetch_page(page)
                .await
                .with_context(|| format!("GitLab issues page {page}"))?;

            let fetched = page_issues.len();
            all_issues.extend(page_issues);

            debug!(fetched, total = all_issues.len(), page, "Fetched page of GitLab issues");

            if fetched < 100 {
                // Fewer than per_page results means this is the last page.
                break;
            }
            page += 1;
        }

        Ok(map_issues(all_issues))
    }

    fn source_type(&self) -> &'static str {
        "gitlab_issues"
    }
}

// ---------------------------------------------------------------------------
// GitlabMergeRequestSource
// ---------------------------------------------------------------------------

/// Fetches GitLab merge requests via the REST API v4.
///
/// # Filters
///
/// - `state` — `opened`/`closed`/`merged`/`all`
/// - `labels` — comma-separated label names
/// - `assignees` — filter by `assignee_username` (first entry used)
///
/// # Pagination
///
/// Iterates through all pages (`per_page=100`) until no more results.
pub struct GitlabMergeRequestSource {
    owner: String,
    repo: String,
    labels: Vec<String>,
    state: String,
    assignees: Option<Vec<String>>,
    /// Pre-resolved token. Never logged.
    token: String,
    /// GitLab base URL.
    base_url: String,
    client: reqwest::Client,
}

impl std::fmt::Debug for GitlabMergeRequestSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitlabMergeRequestSource")
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .field("labels", &self.labels)
            .field("state", &self.state)
            .field("assignees", &self.assignees)
            .field("token", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl GitlabMergeRequestSource {
    /// Create a new source, resolving the GitLab token immediately.
    pub fn new(
        owner: String,
        repo: String,
        labels: Vec<String>,
        state: String,
        assignees: Option<Vec<String>>,
    ) -> Result<Self> {
        let config = GitlabConfig::resolve()?;
        Ok(Self {
            owner,
            repo,
            labels,
            state,
            assignees,
            token: config.token().to_string(),
            base_url: config.base_url().to_string(),
            client: reqwest::Client::new(),
        })
    }

    /// Create a source with an explicit token and base URL (for testing).
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn new_with_config(
        owner: String,
        repo: String,
        labels: Vec<String>,
        state: String,
        assignees: Option<Vec<String>>,
        token: String,
        base_url: String,
    ) -> Self {
        Self {
            owner,
            repo,
            labels,
            state,
            assignees,
            token,
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch a single page of merge requests.
    async fn fetch_page(&self, page: u32) -> Result<Vec<GitlabMergeRequest>> {
        let project_id = format!("{}/{}", self.owner, self.repo);
        let encoded = urlencoding::encode(&project_id);
        let url = format!("{}/api/v4/projects/{}/merge_requests", self.base_url, encoded);

        let mut request = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .query(&[("state", self.state.as_str()), ("per_page", "100")])
            .query(&[("page", page)]);

        if !self.labels.is_empty() {
            request = request.query(&[("labels", self.labels.join(","))]);
        }
        // GitLab supports `assignee_username` filter for MRs (single value).
        if let Some(assignees) = &self.assignees {
            if let Some(first) = assignees.first() {
                request = request.query(&[("assignee_username", first.as_str())]);
            }
        }

        let response = request.send().await.context("Failed to connect to GitLab API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("GitLab API returned HTTP {}: {}", status, body.trim());
        }

        let mrs: Vec<GitlabMergeRequest> =
            response.json().await.context("Failed to parse GitLab merge requests response")?;
        Ok(mrs)
    }
}

#[async_trait]
impl TaskSource for GitlabMergeRequestSource {
    async fn fetch_tasks(&self) -> Result<Vec<Task>> {
        debug!(
            owner = %self.owner,
            repo = %self.repo,
            state = %self.state,
            "Fetching GitLab merge requests"
        );

        let mut all_mrs: Vec<GitlabMergeRequest> = Vec::new();
        let mut page = 1u32;

        loop {
            let page_mrs = self
                .fetch_page(page)
                .await
                .with_context(|| format!("GitLab merge requests page {page}"))?;

            let fetched = page_mrs.len();
            all_mrs.extend(page_mrs);

            debug!(fetched, total = all_mrs.len(), page, "Fetched page of GitLab merge requests");

            if fetched < 100 {
                break;
            }
            page += 1;
        }

        Ok(map_merge_requests(all_mrs))
    }

    fn source_type(&self) -> &'static str {
        "gitlab_merge_requests"
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn map_issues(issues: Vec<GitlabIssue>) -> Vec<Task> {
    issues.into_iter().map(map_issue).collect()
}

fn map_issue(issue: GitlabIssue) -> Task {
    let mut metadata = HashMap::new();
    metadata.insert("gitlab_project_id".to_string(), issue.project_id.to_string());
    metadata.insert("gitlab_iid".to_string(), issue.iid.to_string());
    metadata.insert("state".to_string(), issue.state.clone());

    Task {
        source_id: issue.iid.to_string(),
        title: issue.title,
        body: issue.description.unwrap_or_default(),
        url: issue.web_url,
        labels: issue.labels,
        assignee: issue.assignees.first().map(|u| u.username.clone()),
        metadata,
    }
}

fn map_merge_requests(mrs: Vec<GitlabMergeRequest>) -> Vec<Task> {
    mrs.into_iter().map(map_merge_request).collect()
}

fn map_merge_request(mr: GitlabMergeRequest) -> Task {
    let mut metadata = HashMap::new();
    metadata.insert("gitlab_project_id".to_string(), mr.project_id.to_string());
    metadata.insert("gitlab_iid".to_string(), mr.iid.to_string());
    metadata.insert("state".to_string(), mr.state.clone());
    metadata.insert("source_branch".to_string(), mr.source_branch.clone());
    metadata.insert("target_branch".to_string(), mr.target_branch.clone());
    if let Some(ms) = &mr.merge_status {
        metadata.insert("merge_status".to_string(), ms.clone());
    }
    metadata.insert("draft".to_string(), mr.draft.to_string());

    Task {
        source_id: mr.iid.to_string(),
        title: mr.title,
        body: mr.description.unwrap_or_default(),
        url: mr.web_url,
        labels: mr.labels,
        assignee: mr.assignees.first().map(|u| u.username.clone()),
        metadata,
    }
}

/// Parse a JSON array of GitLab issue objects into [`Task`] structs.
///
/// Useful for testing and offline use, following the pattern established by
/// `parse_gh_issues()` in `github.rs`.
#[allow(dead_code)]
pub fn parse_gitlab_issues(json: &str) -> Result<Vec<Task>> {
    let issues: Vec<GitlabIssue> = serde_json::from_str(json)?;
    Ok(map_issues(issues))
}

/// Parse a JSON array of GitLab merge request objects into [`Task`] structs.
///
/// Useful for testing and offline use.
#[allow(dead_code)]
pub fn parse_gitlab_merge_requests(json: &str) -> Result<Vec<Task>> {
    let mrs: Vec<GitlabMergeRequest> = serde_json::from_str(json)?;
    Ok(map_merge_requests(mrs))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Shared env-var helpers
    // -------------------------------------------------------------------------

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
    // GitlabConfig tests
    // -------------------------------------------------------------------------

    #[test]
    fn is_configured_returns_false_when_env_unset() {
        with_env_lock(|| {
            let _token = EnvRestorer::unset("AGENTD_GITLAB_TOKEN");
            let tmp = std::env::temp_dir().join("agentd_gitlab_test_nonexistent.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());
            assert!(!GitlabConfig::is_configured());
        });
    }

    #[test]
    fn is_configured_returns_true_when_env_set() {
        with_env_lock(|| {
            let _token = EnvRestorer::set("AGENTD_GITLAB_TOKEN", "glpat-testtoken");
            assert!(GitlabConfig::is_configured());
        });
    }

    #[test]
    fn resolve_succeeds_when_env_set() {
        with_env_lock(|| {
            let _token = EnvRestorer::set("AGENTD_GITLAB_TOKEN", "glpat-testtoken123");
            let cfg = GitlabConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.token(), "glpat-testtoken123");
            assert_eq!(cfg.base_url(), DEFAULT_GITLAB_URL);
        });
    }

    #[test]
    fn resolve_uses_custom_url_from_env() {
        with_env_lock(|| {
            let _token = EnvRestorer::set("AGENTD_GITLAB_TOKEN", "glpat-tok");
            let _url = EnvRestorer::set("AGENTD_GITLAB_URL", "https://gitlab.example.com");
            let cfg = GitlabConfig::resolve().expect("should resolve");
            assert_eq!(cfg.base_url(), "https://gitlab.example.com");
        });
    }

    #[test]
    fn resolve_fails_when_no_token_available() {
        with_env_lock(|| {
            let _token = EnvRestorer::unset("AGENTD_GITLAB_TOKEN");
            let tmp = std::env::temp_dir().join("agentd_gitlab_test_nonexistent2.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());
            let err = GitlabConfig::resolve().expect_err("should fail without token");
            let msg = err.to_string();
            assert!(msg.contains("AGENTD_GITLAB_TOKEN"), "message: {msg}");
        });
    }

    #[test]
    fn resolve_reads_token_from_config_file() {
        with_env_lock(|| {
            let _token = EnvRestorer::unset("AGENTD_GITLAB_TOKEN");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[gitlab]\ntoken = \"glpat-fromfile\"\n").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());
            let cfg = GitlabConfig::resolve().expect("should resolve from file");
            assert_eq!(cfg.token(), "glpat-fromfile");
        });
    }

    #[test]
    fn resolve_env_takes_precedence_over_file() {
        with_env_lock(|| {
            let _token = EnvRestorer::set("AGENTD_GITLAB_TOKEN", "glpat-fromenv");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[gitlab]\ntoken = \"glpat-fromfile\"\n").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());
            let cfg = GitlabConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.token(), "glpat-fromenv");
        });
    }

    #[test]
    fn debug_does_not_expose_token() {
        with_env_lock(|| {
            let _token = EnvRestorer::set("AGENTD_GITLAB_TOKEN", "glpat-secret");
            let cfg = GitlabConfig::resolve().unwrap();
            let debug_str = format!("{cfg:?}");
            assert!(debug_str.contains("<redacted>"));
            assert!(!debug_str.contains("glpat-secret"));
        });
    }

    // -------------------------------------------------------------------------
    // parse_gitlab_issues tests
    // -------------------------------------------------------------------------

    fn full_issue_json() -> &'static str {
        r#"[
            {
                "iid": 42,
                "title": "Fix the widget",
                "description": "The widget is broken.",
                "web_url": "https://gitlab.com/myorg/myrepo/-/issues/42",
                "state": "opened",
                "labels": ["bug", "agent"],
                "assignees": [{"username": "alice"}],
                "project_id": 12345
            },
            {
                "iid": 43,
                "title": "Add feature X",
                "description": null,
                "web_url": "https://gitlab.com/myorg/myrepo/-/issues/43",
                "state": "opened",
                "labels": [],
                "assignees": [],
                "project_id": 12345
            }
        ]"#
    }

    #[test]
    fn test_parse_gitlab_issues_full() {
        let tasks = parse_gitlab_issues(full_issue_json()).unwrap();
        assert_eq!(tasks.len(), 2);

        let t0 = &tasks[0];
        assert_eq!(t0.source_id, "42");
        assert_eq!(t0.title, "Fix the widget");
        assert_eq!(t0.body, "The widget is broken.");
        assert_eq!(t0.url, "https://gitlab.com/myorg/myrepo/-/issues/42");
        assert_eq!(t0.labels, vec!["bug", "agent"]);
        assert_eq!(t0.assignee, Some("alice".to_string()));
        assert_eq!(t0.metadata.get("gitlab_project_id").unwrap(), "12345");
        assert_eq!(t0.metadata.get("gitlab_iid").unwrap(), "42");
        assert_eq!(t0.metadata.get("state").unwrap(), "opened");
    }

    #[test]
    fn test_parse_gitlab_issues_null_description() {
        let tasks = parse_gitlab_issues(full_issue_json()).unwrap();
        let t1 = &tasks[1];
        assert_eq!(t1.source_id, "43");
        assert_eq!(t1.body, "");
        assert!(t1.assignee.is_none());
        assert!(t1.labels.is_empty());
    }

    #[test]
    fn test_parse_gitlab_issues_empty() {
        let tasks = parse_gitlab_issues("[]").unwrap();
        assert!(tasks.is_empty());
    }

    // -------------------------------------------------------------------------
    // parse_gitlab_merge_requests tests
    // -------------------------------------------------------------------------

    fn full_mr_json() -> &'static str {
        r#"[
            {
                "iid": 15,
                "title": "Add new feature",
                "description": "This MR adds a new feature.",
                "web_url": "https://gitlab.com/myorg/myrepo/-/merge_requests/15",
                "state": "opened",
                "labels": ["enhancement"],
                "assignees": [{"username": "bob"}],
                "source_branch": "feature/new-thing",
                "target_branch": "main",
                "merge_status": "can_be_merged",
                "draft": false,
                "project_id": 12345
            },
            {
                "iid": 16,
                "title": "WIP: cleanup",
                "description": null,
                "web_url": "https://gitlab.com/myorg/myrepo/-/merge_requests/16",
                "state": "opened",
                "labels": [],
                "assignees": [],
                "source_branch": "cleanup/stuff",
                "target_branch": "main",
                "merge_status": null,
                "draft": true,
                "project_id": 12345
            }
        ]"#
    }

    #[test]
    fn test_parse_gitlab_merge_requests_full() {
        let tasks = parse_gitlab_merge_requests(full_mr_json()).unwrap();
        assert_eq!(tasks.len(), 2);

        let t0 = &tasks[0];
        assert_eq!(t0.source_id, "15");
        assert_eq!(t0.title, "Add new feature");
        assert_eq!(t0.body, "This MR adds a new feature.");
        assert_eq!(t0.url, "https://gitlab.com/myorg/myrepo/-/merge_requests/15");
        assert_eq!(t0.labels, vec!["enhancement"]);
        assert_eq!(t0.assignee, Some("bob".to_string()));
        assert_eq!(t0.metadata.get("gitlab_project_id").unwrap(), "12345");
        assert_eq!(t0.metadata.get("gitlab_iid").unwrap(), "15");
        assert_eq!(t0.metadata.get("source_branch").unwrap(), "feature/new-thing");
        assert_eq!(t0.metadata.get("target_branch").unwrap(), "main");
        assert_eq!(t0.metadata.get("merge_status").unwrap(), "can_be_merged");
        assert_eq!(t0.metadata.get("draft").unwrap(), "false");
        assert_eq!(t0.metadata.get("state").unwrap(), "opened");
    }

    #[test]
    fn test_parse_gitlab_merge_requests_draft() {
        let tasks = parse_gitlab_merge_requests(full_mr_json()).unwrap();
        let t1 = &tasks[1];
        assert_eq!(t1.source_id, "16");
        assert_eq!(t1.body, "");
        assert!(t1.assignee.is_none());
        assert_eq!(t1.metadata.get("draft").unwrap(), "true");
        // null merge_status should not be in metadata
        assert!(!t1.metadata.contains_key("merge_status"));
    }

    #[test]
    fn test_parse_gitlab_merge_requests_empty() {
        let tasks = parse_gitlab_merge_requests("[]").unwrap();
        assert!(tasks.is_empty());
    }

    // -------------------------------------------------------------------------
    // source_type tests
    // -------------------------------------------------------------------------

    #[test]
    fn gitlab_issue_source_type() {
        let src = GitlabIssueSource::new_with_config(
            "org".into(),
            "repo".into(),
            vec![],
            "opened".into(),
            None,
            "tok".into(),
            DEFAULT_GITLAB_URL.into(),
        );
        assert_eq!(src.source_type(), "gitlab_issues");
    }

    #[test]
    fn gitlab_mr_source_type() {
        let src = GitlabMergeRequestSource::new_with_config(
            "org".into(),
            "repo".into(),
            vec![],
            "opened".into(),
            None,
            "tok".into(),
            DEFAULT_GITLAB_URL.into(),
        );
        assert_eq!(src.source_type(), "gitlab_merge_requests");
    }

    #[test]
    fn gitlab_issue_source_debug_redacts_token() {
        let src = GitlabIssueSource::new_with_config(
            "org".into(),
            "repo".into(),
            vec![],
            "opened".into(),
            None,
            "glpat-secret".into(),
            DEFAULT_GITLAB_URL.into(),
        );
        let debug_str = format!("{src:?}");
        assert!(debug_str.contains("<redacted>"));
        assert!(!debug_str.contains("glpat-secret"));
    }
}
