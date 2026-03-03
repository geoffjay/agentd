//! Apply command for declarative workflow and agent configuration.
//!
//! Reads YAML workflow template files and applies them to the orchestrator,
//! resolving agent names to UUIDs automatically.
//!
//! # Examples
//!
//! ```bash
//! # Apply a single workflow template
//! agent apply .agentd/workflows/issue-worker.yml
//!
//! # Dry-run (validate only, don't create)
//! agent apply --dry-run .agentd/workflows/issue-worker.yml
//! ```

use anyhow::{bail, Context, Result};
use colored::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use orchestrator::client::OrchestratorClient;
use orchestrator::scheduler::types::{CreateWorkflowRequest, TaskSourceConfig};
use orchestrator::types::{AgentResponse, ToolPolicy};

/// YAML workflow template — references agents by name, not UUID.
#[derive(Debug, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    /// Agent name (resolved to UUID at apply time).
    pub agent: String,
    pub source: SourceTemplate,
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub tool_policy: Option<ToolPolicy>,
    /// Inline prompt template with {{variables}}.
    pub prompt_template: Option<String>,
    /// Path to an external prompt template file (relative to the YAML file).
    pub prompt_template_file: Option<String>,
}

fn default_poll_interval() -> u64 {
    60
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceTemplate {
    GithubIssues {
        owner: String,
        repo: String,
        #[serde(default)]
        labels: Vec<String>,
        #[serde(default = "default_state")]
        state: String,
    },
}

fn default_state() -> String {
    "open".to_string()
}

/// Parse a YAML workflow template file.
pub fn parse_workflow_template(path: &Path) -> Result<WorkflowTemplate> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read template file: {}", path.display()))?;
    let template: WorkflowTemplate = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML template: {}", path.display()))?;

    // Resolve prompt_template_file relative to the YAML file's directory
    if template.prompt_template.is_none() && template.prompt_template_file.is_none() {
        bail!(
            "Workflow template '{}' must have either 'prompt_template' or 'prompt_template_file'",
            path.display()
        );
    }

    Ok(template)
}

/// Resolve the prompt template from inline or file reference.
fn resolve_prompt(template: &WorkflowTemplate, yaml_path: &Path) -> Result<String> {
    if let Some(ref inline) = template.prompt_template {
        return Ok(inline.clone());
    }

    if let Some(ref file_path) = template.prompt_template_file {
        let base_dir = yaml_path.parent().unwrap_or(Path::new("."));
        let full_path = base_dir.join(file_path);
        return std::fs::read_to_string(&full_path).with_context(|| {
            format!(
                "Failed to read prompt template file: {} (resolved from {})",
                full_path.display(),
                file_path
            )
        });
    }

    bail!("No prompt template specified");
}

/// Resolve an agent name to a UUID by querying the orchestrator API.
async fn resolve_agent_by_name(
    client: &OrchestratorClient,
    name: &str,
) -> Result<AgentResponse> {
    let response = client
        .list_agents(None)
        .await
        .context("Failed to list agents for name resolution")?;

    let matches: Vec<&AgentResponse> = response.items.iter().filter(|a| a.name == name).collect();

    match matches.len() {
        0 => bail!(
            "Agent '{}' not found. Use 'agent orchestrator list-agents' to see available agents.",
            name
        ),
        1 => Ok(matches[0].clone()),
        n => bail!(
            "Found {} agents named '{}'. Workflow templates require unique agent names.",
            n,
            name
        ),
    }
}

/// Apply a workflow template file to the orchestrator.
pub async fn apply_workflow(
    client: &OrchestratorClient,
    path: &Path,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let template = parse_workflow_template(path)?;
    let prompt = resolve_prompt(&template, path)?;

    if !json {
        println!(
            "{} {}",
            "Applying workflow:".blue().bold(),
            template.name.bright_white()
        );
        println!("  {}: {}", "Agent".bold(), template.agent);
        println!("  {}: {}", "Source".bold(), describe_source(&template.source));
        println!("  {}: {}s", "Poll interval".bold(), template.poll_interval);
        println!("  {}: {}", "Enabled".bold(), template.enabled);
        if let Some(ref policy) = template.tool_policy {
            println!("  {}: {}", "Tool policy".bold(), policy.mode_str());
        }
    }

    // Resolve agent name → UUID
    if !json {
        print!("  {} ", "Resolving agent name...".bright_black());
    }
    let agent = resolve_agent_by_name(client, &template.agent).await?;
    if !json {
        println!("{} ({})", "found".green(), agent.id);
    }

    if agent.status != orchestrator::types::AgentStatus::Running {
        bail!(
            "Agent '{}' is not running (status: {}). Start the agent before applying workflows.",
            template.agent,
            agent.status
        );
    }

    if dry_run {
        if json {
            let result = serde_json::json!({
                "dry_run": true,
                "valid": true,
                "workflow_name": template.name,
                "agent_name": template.agent,
                "agent_id": agent.id,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!();
            println!("{}", "Dry run: template is valid, no changes made.".yellow());
        }
        return Ok(());
    }

    // Convert SourceTemplate → TaskSourceConfig
    let source_config = match template.source {
        SourceTemplate::GithubIssues { owner, repo, labels, state } => {
            TaskSourceConfig::GithubIssues { owner, repo, labels, state }
        }
    };

    let request = CreateWorkflowRequest {
        name: template.name.clone(),
        agent_id: agent.id,
        source_config,
        prompt_template: prompt,
        poll_interval_secs: template.poll_interval,
        enabled: template.enabled,
        tool_policy: template.tool_policy.unwrap_or_default(),
    };

    let workflow = client
        .create_workflow(&request)
        .await
        .context("Failed to create workflow")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!();
        println!(
            "{}",
            format!("Workflow '{}' created (ID: {})", template.name, workflow.id)
                .green()
                .bold()
        );
    }

    Ok(())
}

/// Apply all YAML files in a directory.
pub async fn apply_directory(
    client: &OrchestratorClient,
    dir: &Path,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let mut yaml_files: Vec<PathBuf> = Vec::new();

    // Collect all .yml and .yaml files
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yml" || ext == "yaml" {
                    yaml_files.push(path);
                }
            }
        }
    }

    yaml_files.sort();

    if yaml_files.is_empty() {
        bail!("No .yml or .yaml files found in: {}", dir.display());
    }

    if !json {
        println!(
            "{} {} file(s) in {}",
            "Applying".blue().bold(),
            yaml_files.len(),
            dir.display()
        );
        println!();
    }

    for path in &yaml_files {
        apply_workflow(client, path, dry_run, json).await?;
        if !json {
            println!();
        }
    }

    Ok(())
}

fn describe_source(source: &SourceTemplate) -> String {
    match source {
        SourceTemplate::GithubIssues { owner, repo, labels, .. } => {
            let label_str = if labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", labels.join(", "))
            };
            format!("github_issues {}/{}{}", owner, repo, label_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workflow_template() {
        let yaml = r#"
name: test-workflow
agent: my-agent
source:
  type: github_issues
  owner: org
  repo: repo
  labels:
    - bug
prompt_template: "Fix: {{title}}"
"#;
        let template: WorkflowTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.name, "test-workflow");
        assert_eq!(template.agent, "my-agent");
        assert_eq!(template.poll_interval, 60);
        assert!(template.enabled);
        assert_eq!(template.prompt_template.unwrap(), "Fix: {{title}}");
    }

    #[test]
    fn test_parse_workflow_with_tool_policy() {
        let yaml = r#"
name: safe-review
agent: reviewer
source:
  type: github_issues
  owner: org
  repo: repo
tool_policy:
  mode: allow_list
  tools:
    - Read
    - Grep
prompt_template: "Review: {{title}}"
"#;
        let template: WorkflowTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.name, "safe-review");
        let policy = template.tool_policy.unwrap();
        assert_eq!(policy, ToolPolicy::AllowList {
            tools: vec!["Read".to_string(), "Grep".to_string()]
        });
    }

    #[test]
    fn test_parse_workflow_defaults() {
        let yaml = r#"
name: minimal
agent: worker
source:
  type: github_issues
  owner: org
  repo: repo
prompt_template: "Do: {{title}}"
"#;
        let template: WorkflowTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.poll_interval, 60);
        assert!(template.enabled);
        assert!(template.tool_policy.is_none());
    }

    #[test]
    fn test_parse_source_github_issues() {
        let yaml = r#"
type: github_issues
owner: myorg
repo: myrepo
labels:
  - bug
  - agent
state: open
"#;
        let source: SourceTemplate = serde_yaml::from_str(yaml).unwrap();
        match source {
            SourceTemplate::GithubIssues { owner, repo, labels, state } => {
                assert_eq!(owner, "myorg");
                assert_eq!(repo, "myrepo");
                assert_eq!(labels, vec!["bug", "agent"]);
                assert_eq!(state, "open");
            }
        }
    }

    #[test]
    fn test_describe_source() {
        let source = SourceTemplate::GithubIssues {
            owner: "org".to_string(),
            repo: "repo".to_string(),
            labels: vec!["bug".to_string()],
            state: "open".to_string(),
        };
        let desc = describe_source(&source);
        assert_eq!(desc, "github_issues org/repo [bug]");
    }
}
