use crate::scheduler::source::TaskSource;
use crate::scheduler::types::Task;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn};

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
            "gh".to_string(),
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
}
