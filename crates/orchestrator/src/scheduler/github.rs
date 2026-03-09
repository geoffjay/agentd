use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;
use tracing::{debug, warn};

/// Resolve the absolute path to the `gh` CLI at process start.
///
/// macOS GUI apps inherit a minimal PATH that typically doesn't include
/// Homebrew or other user-installed binary directories. We probe common
/// locations so the orchestrator works whether launched from a terminal or
/// from an app bundle.
static GH_PATH: LazyLock<String> = LazyLock::new(|| {
    // Prefer an explicit override.
    if let Ok(p) = std::env::var("GH_PATH") {
        return p;
    }

    let candidates = ["/opt/homebrew/bin/gh", "/usr/local/bin/gh", "/usr/bin/gh"];
    for path in candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    // Fall back to bare name and let the OS resolve it.
    "gh".to_string()
});

/// Fetches GitHub Issues via the `gh` CLI.
pub struct GithubIssueSource {
    owner: String,
    repo: String,
    labels: Vec<String>,
    state: String,
}

impl GithubIssueSource {
    pub fn new(owner: String, repo: String, labels: Vec<String>, state: String) -> Self {
        Self { owner, repo, labels, state }
    }
}

#[derive(Debug, Deserialize)]
struct GhIssue {
    number: u64,
    title: String,
    body: Option<String>,
    url: String,
    labels: Vec<GhLabel>,
    assignees: Vec<GhAssignee>,
}

#[derive(Debug, Deserialize)]
struct GhLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GhAssignee {
    login: String,
}

#[async_trait]
impl TaskSource for GithubIssueSource {
    async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>> {
        let mut args = vec![
            GH_PATH.clone(),
            "issue".to_string(),
            "list".to_string(),
            "--json".to_string(),
            "number,title,body,url,labels,assignees".to_string(),
            "--repo".to_string(),
            format!("{}/{}", self.owner, self.repo),
            "--state".to_string(),
            self.state.clone(),
        ];

        for label in &self.labels {
            args.push("--label".to_string());
            args.push(label.clone());
        }

        debug!(repo = %format!("{}/{}", self.owner, self.repo), "Fetching GitHub issues");

        let output = tokio::process::Command::new(&args[0]).args(&args[1..]).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(stderr = %stderr, "gh CLI returned non-zero exit code");
            anyhow::bail!("gh issue list failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let issues: Vec<GhIssue> = serde_json::from_str(&stdout)?;

        let tasks = issues
            .into_iter()
            .map(|issue| Task {
                source_id: issue.number.to_string(),
                title: issue.title,
                body: issue.body.unwrap_or_default(),
                url: issue.url,
                labels: issue.labels.into_iter().map(|l| l.name).collect(),
                assignee: issue.assignees.first().map(|a| a.login.clone()),
                metadata: HashMap::new(),
            })
            .collect();

        Ok(tasks)
    }

    fn source_type(&self) -> &'static str {
        "github_issues"
    }
}

/// Fetches GitHub Pull Requests via the `gh` CLI.
pub struct GithubPullRequestSource {
    owner: String,
    repo: String,
    labels: Vec<String>,
    state: String,
}

impl GithubPullRequestSource {
    pub fn new(owner: String, repo: String, labels: Vec<String>, state: String) -> Self {
        Self { owner, repo, labels, state }
    }
}

#[derive(Debug, Deserialize)]
struct GhPullRequest {
    number: u64,
    title: String,
    body: Option<String>,
    url: String,
    labels: Vec<GhLabel>,
    assignees: Vec<GhAssignee>,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
}

#[async_trait]
impl TaskSource for GithubPullRequestSource {
    async fn fetch_tasks(&self) -> anyhow::Result<Vec<Task>> {
        let mut args = vec![
            GH_PATH.clone(),
            "pr".to_string(),
            "list".to_string(),
            "--json".to_string(),
            "number,title,body,url,labels,assignees,headRefName,baseRefName,isDraft".to_string(),
            "--repo".to_string(),
            format!("{}/{}", self.owner, self.repo),
            "--state".to_string(),
            self.state.clone(),
        ];

        for label in &self.labels {
            args.push("--label".to_string());
            args.push(label.clone());
        }

        debug!(repo = %format!("{}/{}", self.owner, self.repo), "Fetching GitHub pull requests");

        let output = tokio::process::Command::new(&args[0]).args(&args[1..]).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(stderr = %stderr, "gh CLI returned non-zero exit code");
            anyhow::bail!("gh pr list failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout)?;
        let prs: Vec<GhPullRequest> = serde_json::from_str(&stdout)?;

        let tasks = prs
            .into_iter()
            .map(|pr| {
                let mut metadata = HashMap::new();
                metadata.insert("head_ref".to_string(), pr.head_ref_name);
                metadata.insert("base_ref".to_string(), pr.base_ref_name);
                metadata.insert("is_draft".to_string(), pr.is_draft.to_string());

                Task {
                    source_id: pr.number.to_string(),
                    title: pr.title,
                    body: pr.body.unwrap_or_default(),
                    url: pr.url,
                    labels: pr.labels.into_iter().map(|l| l.name).collect(),
                    assignee: pr.assignees.first().map(|a| a.login.clone()),
                    metadata,
                }
            })
            .collect();

        Ok(tasks)
    }

    fn source_type(&self) -> &'static str {
        "github_pull_requests"
    }
}

/// Parse `gh issue list --json` output into Tasks (useful for testing).
#[allow(dead_code)]
pub fn parse_gh_issues(json: &str) -> anyhow::Result<Vec<Task>> {
    let issues: Vec<GhIssue> = serde_json::from_str(json)?;
    Ok(issues
        .into_iter()
        .map(|issue| Task {
            source_id: issue.number.to_string(),
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            url: issue.url,
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
            assignee: issue.assignees.first().map(|a| a.login.clone()),
            metadata: HashMap::new(),
        })
        .collect())
}

/// Parse `gh pr list --json` output into Tasks (useful for testing).
#[allow(dead_code)]
pub fn parse_gh_pull_requests(json: &str) -> anyhow::Result<Vec<Task>> {
    let prs: Vec<GhPullRequest> = serde_json::from_str(json)?;
    Ok(prs
        .into_iter()
        .map(|pr| {
            let mut metadata = HashMap::new();
            metadata.insert("head_ref".to_string(), pr.head_ref_name);
            metadata.insert("base_ref".to_string(), pr.base_ref_name);
            metadata.insert("is_draft".to_string(), pr.is_draft.to_string());

            Task {
                source_id: pr.number.to_string(),
                title: pr.title,
                body: pr.body.unwrap_or_default(),
                url: pr.url,
                labels: pr.labels.into_iter().map(|l| l.name).collect(),
                assignee: pr.assignees.first().map(|a| a.login.clone()),
                metadata,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gh_issues() {
        let json = r#"[
            {
                "number": 1,
                "title": "Fix the widget",
                "body": "The widget is broken.",
                "url": "https://github.com/org/repo/issues/1",
                "labels": [{"name": "bug"}, {"name": "agent"}],
                "assignees": [{"login": "alice"}]
            },
            {
                "number": 5,
                "title": "Add feature X",
                "body": null,
                "url": "https://github.com/org/repo/issues/5",
                "labels": [],
                "assignees": []
            }
        ]"#;

        let tasks = parse_gh_issues(json).unwrap();
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].source_id, "1");
        assert_eq!(tasks[0].title, "Fix the widget");
        assert_eq!(tasks[0].body, "The widget is broken.");
        assert_eq!(tasks[0].labels, vec!["bug", "agent"]);
        assert_eq!(tasks[0].assignee, Some("alice".to_string()));

        assert_eq!(tasks[1].source_id, "5");
        assert_eq!(tasks[1].body, "");
        assert!(tasks[1].assignee.is_none());
    }

    #[test]
    fn test_parse_empty() {
        let tasks = parse_gh_issues("[]").unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_gh_pull_requests() {
        let json = r#"[
            {
                "number": 42,
                "title": "Add new feature",
                "body": "This PR adds a new feature.",
                "url": "https://github.com/org/repo/pull/42",
                "labels": [{"name": "enhancement"}, {"name": "agent"}],
                "assignees": [{"login": "bob"}],
                "headRefName": "feature/new-thing",
                "baseRefName": "main",
                "isDraft": false
            },
            {
                "number": 43,
                "title": "WIP: refactor",
                "body": null,
                "url": "https://github.com/org/repo/pull/43",
                "labels": [],
                "assignees": [],
                "headRefName": "refactor/cleanup",
                "baseRefName": "main",
                "isDraft": true
            }
        ]"#;

        let tasks = parse_gh_pull_requests(json).unwrap();
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].source_id, "42");
        assert_eq!(tasks[0].title, "Add new feature");
        assert_eq!(tasks[0].body, "This PR adds a new feature.");
        assert_eq!(tasks[0].labels, vec!["enhancement", "agent"]);
        assert_eq!(tasks[0].assignee, Some("bob".to_string()));
        assert_eq!(tasks[0].metadata.get("head_ref").unwrap(), "feature/new-thing");
        assert_eq!(tasks[0].metadata.get("base_ref").unwrap(), "main");
        assert_eq!(tasks[0].metadata.get("is_draft").unwrap(), "false");

        assert_eq!(tasks[1].source_id, "43");
        assert_eq!(tasks[1].body, "");
        assert!(tasks[1].assignee.is_none());
        assert_eq!(tasks[1].metadata.get("is_draft").unwrap(), "true");
    }

    #[test]
    fn test_parse_pull_requests_empty() {
        let tasks = parse_gh_pull_requests("[]").unwrap();
        assert!(tasks.is_empty());
    }
}
