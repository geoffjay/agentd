//! Apply and teardown commands for declarative .agentd/ project directories.
//!
//! Reads YAML template files for agents and workflows, applying them in the
//! correct dependency order: agents first, then workflows (which reference
//! agents by name).
//!
//! # Directory Convention
//!
//! ```text
//! .agentd/
//!   agents/
//!     worker.yml
//!   workflows/
//!     issue-worker.yml    # references agent: worker
//! ```

use anyhow::{bail, Context, Result};
use colored::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use orchestrator::client::OrchestratorClient;
use orchestrator::scheduler::types::{CreateWorkflowRequest, TaskSourceConfig};
use orchestrator::types::{AgentResponse, AgentStatus, CreateAgentRequest, ToolPolicy};

// ── YAML template types ──────────────────────────────────────────────

/// YAML agent template (`.agentd/agents/<name>.yml`).
#[derive(Debug, Deserialize)]
pub struct AgentTemplate {
    pub name: String,
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_shell")]
    pub shell: String,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub worktree: bool,
    pub prompt: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tool_policy: ToolPolicy,
}

fn default_working_dir() -> String {
    ".".to_string()
}
fn default_shell() -> String {
    "zsh".to_string()
}

/// YAML workflow template (`.agentd/workflows/<name>.yml`).
#[derive(Debug, Deserialize)]
pub struct WorkflowTemplate {
    pub name: String,
    /// Agent name — resolved to UUID at apply time.
    pub agent: String,
    pub source: SourceTemplate,
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub tool_policy: Option<ToolPolicy>,
    pub prompt_template: Option<String>,
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

// ── Parsing helpers ──────────────────────────────────────────────────

fn parse_agent_template(path: &Path) -> Result<AgentTemplate> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse agent template: {}", path.display()))
}

fn parse_workflow_template(path: &Path) -> Result<WorkflowTemplate> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;
    let tmpl: WorkflowTemplate = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse workflow template: {}", path.display()))?;
    if tmpl.prompt_template.is_none() && tmpl.prompt_template_file.is_none() {
        bail!(
            "Workflow '{}' must have either 'prompt_template' or 'prompt_template_file'",
            path.display()
        );
    }
    Ok(tmpl)
}

fn resolve_prompt(tmpl: &WorkflowTemplate, yaml_path: &Path) -> Result<String> {
    if let Some(ref t) = tmpl.prompt_template {
        return Ok(t.clone());
    }
    if let Some(ref file) = tmpl.prompt_template_file {
        let base = yaml_path.parent().unwrap_or(Path::new("."));
        let full = base.join(file);
        return std::fs::read_to_string(&full)
            .with_context(|| format!("Failed to read prompt file: {}", full.display()));
    }
    bail!("No prompt template specified");
}

fn collect_yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("Failed to read: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "yml" || ext == "yaml" {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

// ── Agent resolution helpers ─────────────────────────────────────────

async fn find_agent_by_name(
    client: &OrchestratorClient,
    name: &str,
) -> Result<Option<AgentResponse>> {
    let resp = client.list_agents(None).await?;
    let matches: Vec<&AgentResponse> = resp.items.iter().filter(|a| a.name == name).collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0].clone())),
        n => bail!("Found {} agents named '{}'. Names must be unique.", n, name),
    }
}

async fn wait_for_agent_running(
    client: &OrchestratorClient,
    name: &str,
    timeout_secs: u64,
) -> Result<AgentResponse> {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    loop {
        if let Some(agent) = find_agent_by_name(client, name).await? {
            if agent.status == AgentStatus::Running {
                return Ok(agent);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "Timeout waiting for agent '{}' to reach running status (waited {}s)",
                name,
                timeout_secs
            );
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

// ── Apply: single workflow file ──────────────────────────────────────

pub async fn apply_workflow_file(
    client: &OrchestratorClient,
    path: &Path,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let tmpl = parse_workflow_template(path)?;
    let prompt = resolve_prompt(&tmpl, path)?;

    if !json {
        println!(
            "  {} workflow '{}'...",
            if dry_run { "Validating" } else { "Creating" }.cyan(),
            tmpl.name.bright_white()
        );
    }

    // Resolve agent name → UUID
    let agent = find_agent_by_name(client, &tmpl.agent).await?.ok_or_else(|| {
        anyhow::anyhow!("Agent '{}' not found (referenced by workflow '{}')", tmpl.agent, tmpl.name)
    })?;

    if agent.status != AgentStatus::Running {
        bail!(
            "Agent '{}' is not running (status: {}). Start it before applying workflow '{}'.",
            tmpl.agent,
            agent.status,
            tmpl.name
        );
    }

    if dry_run {
        if !json {
            println!("    {} (agent '{}' → {})", "valid".green(), tmpl.agent, agent.id);
        }
        return Ok(());
    }

    let source_config = match tmpl.source {
        SourceTemplate::GithubIssues { owner, repo, labels, state } => {
            TaskSourceConfig::GithubIssues { owner, repo, labels, state }
        }
    };

    let request = CreateWorkflowRequest {
        name: tmpl.name.clone(),
        agent_id: agent.id,
        source_config,
        prompt_template: prompt,
        poll_interval_secs: tmpl.poll_interval,
        enabled: tmpl.enabled,
        tool_policy: tmpl.tool_policy.unwrap_or_default(),
    };

    let workflow = client.create_workflow(&request).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&workflow)?);
    } else {
        println!("    {} (ID: {})", "created".green(), workflow.id.to_string().bright_black());
    }

    Ok(())
}

// ── Apply: composite .agentd/ directory ──────────────────────────────

pub async fn apply_directory(
    client: &OrchestratorClient,
    dir: &Path,
    dry_run: bool,
    wait_timeout: u64,
    json: bool,
) -> Result<()> {
    let agents_dir = dir.join("agents");
    let workflows_dir = dir.join("workflows");

    let agent_files = collect_yaml_files(&agents_dir)?;
    let workflow_files = collect_yaml_files(&workflows_dir)?;

    // Also check for loose YAML files in the directory itself (single-type dirs)
    let loose_files = collect_yaml_files(dir)?;
    let has_subdirs = agents_dir.is_dir() || workflows_dir.is_dir();

    if agent_files.is_empty() && workflow_files.is_empty() && loose_files.is_empty() {
        bail!(
            "No YAML templates found in '{}'. Expected agents/ and/or workflows/ subdirectories.",
            dir.display()
        );
    }

    // Phase 0: Parse and validate all templates upfront (fail fast)
    if !json {
        println!("{}", "Validating templates...".blue().bold());
    }

    let mut agent_templates = Vec::new();
    for path in &agent_files {
        let tmpl = parse_agent_template(path)
            .with_context(|| format!("Validation failed for {}", path.display()))?;
        if !json {
            println!("  {} agent '{}'", "ok".green(), tmpl.name);
        }
        agent_templates.push((path.clone(), tmpl));
    }

    let mut workflow_templates = Vec::new();
    for path in &workflow_files {
        let tmpl = parse_workflow_template(path)
            .with_context(|| format!("Validation failed for {}", path.display()))?;
        let _prompt = resolve_prompt(&tmpl, path)?;
        if !json {
            println!("  {} workflow '{}' (agent: {})", "ok".green(), tmpl.name, tmpl.agent);
        }
        workflow_templates.push((path.clone(), tmpl));
    }

    // If no subdirectories, treat loose files as workflows
    if !has_subdirs && !loose_files.is_empty() {
        for path in &loose_files {
            let tmpl = parse_workflow_template(path)
                .with_context(|| format!("Validation failed for {}", path.display()))?;
            let _prompt = resolve_prompt(&tmpl, path)?;
            if !json {
                println!("  {} workflow '{}' (agent: {})", "ok".green(), tmpl.name, tmpl.agent);
            }
            workflow_templates.push((path.clone(), tmpl));
        }
    }

    if dry_run {
        if json {
            let result = serde_json::json!({
                "dry_run": true,
                "valid": true,
                "agents": agent_templates.iter().map(|(_, t)| &t.name).collect::<Vec<_>>(),
                "workflows": workflow_templates.iter().map(|(_, t)| &t.name).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!();
            println!(
                "{} {} agent(s), {} workflow(s)",
                "Dry run passed:".yellow(),
                agent_templates.len(),
                workflow_templates.len()
            );
        }
        return Ok(());
    }

    // Phase 1: Create agents
    if !agent_templates.is_empty() {
        if !json {
            println!();
            println!("{}", "Creating agents...".blue().bold());
        }
        for (path, tmpl) in &agent_templates {
            apply_agent(client, tmpl, path, json).await?;
        }

        // Phase 2: Wait for all agents to reach running status
        if !json {
            println!();
            println!("{}", "Waiting for agents to start...".blue().bold());
        }
        for (_, tmpl) in &agent_templates {
            if !json {
                print!("  {} '{}'... ", "Waiting for".cyan(), tmpl.name);
            }
            let agent = wait_for_agent_running(client, &tmpl.name, wait_timeout).await?;
            if !json {
                println!("{} ({})", "running".green(), agent.id.to_string().bright_black());
            }
        }
    }

    // Phase 3: Create workflows
    if !workflow_templates.is_empty() {
        if !json {
            println!();
            println!("{}", "Creating workflows...".blue().bold());
        }
        for (path, _) in &workflow_templates {
            apply_workflow_file(client, path, false, json).await?;
        }
    }

    // Summary
    if !json {
        println!();
        println!(
            "{}",
            format!(
                "Applied {} agent(s) and {} workflow(s)",
                agent_templates.len(),
                workflow_templates.len()
            )
            .green()
            .bold()
        );
    }

    Ok(())
}

async fn apply_agent(
    client: &OrchestratorClient,
    tmpl: &AgentTemplate,
    path: &Path,
    json: bool,
) -> Result<()> {
    // Check if agent already exists
    if let Some(existing) = find_agent_by_name(client, &tmpl.name).await? {
        if existing.status == AgentStatus::Running {
            if !json {
                println!(
                    "  {} '{}' (already running, ID: {})",
                    "Skipping".yellow(),
                    tmpl.name,
                    existing.id.to_string().bright_black()
                );
            }
            return Ok(());
        }
        bail!(
            "Agent '{}' exists but is {} — use 'agent orchestrator delete-agent {}' first",
            tmpl.name,
            existing.status,
            existing.id
        );
    }

    if !json {
        print!("  {} '{}'... ", "Creating".cyan(), tmpl.name);
    }

    // Resolve working_dir relative to the YAML file
    let working_dir = if tmpl.working_dir == "." {
        std::env::current_dir()?.to_string_lossy().to_string()
    } else {
        let base = path.parent().unwrap_or(Path::new("."));
        let full = base.join(&tmpl.working_dir);
        full.canonicalize().unwrap_or(full).to_string_lossy().to_string()
    };

    let request = CreateAgentRequest {
        name: tmpl.name.clone(),
        working_dir,
        user: None,
        shell: tmpl.shell.clone(),
        interactive: tmpl.interactive,
        prompt: tmpl.prompt.clone(),
        worktree: tmpl.worktree,
        system_prompt: tmpl.system_prompt.clone(),
        tool_policy: tmpl.tool_policy.clone(),
    };

    let agent = client.create_agent(&request).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
    } else {
        println!("{} (ID: {})", "created".green(), agent.id.to_string().bright_black());
    }

    Ok(())
}

// ── Teardown: delete resources in reverse order ──────────────────────

pub async fn teardown_directory(
    client: &OrchestratorClient,
    dir: &Path,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let agents_dir = dir.join("agents");
    let workflows_dir = dir.join("workflows");

    let agent_files = collect_yaml_files(&agents_dir)?;
    let workflow_files = collect_yaml_files(&workflows_dir)?;

    // Parse templates just for names
    let mut agent_names = Vec::new();
    for path in &agent_files {
        let tmpl = parse_agent_template(path)?;
        agent_names.push(tmpl.name);
    }

    let mut workflow_names = Vec::new();
    for path in &workflow_files {
        let tmpl = parse_workflow_template(path)?;
        workflow_names.push(tmpl.name);
    }

    if dry_run {
        if json {
            let result = serde_json::json!({
                "dry_run": true,
                "workflows_to_delete": workflow_names,
                "agents_to_delete": agent_names,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("{}", "Dry run — would delete:".yellow());
            for name in &workflow_names {
                println!("  workflow: {}", name);
            }
            for name in &agent_names {
                println!("  agent: {}", name);
            }
        }
        return Ok(());
    }

    // Delete workflows first (they depend on agents)
    if !workflow_names.is_empty() {
        if !json {
            println!("{}", "Deleting workflows...".blue().bold());
        }
        let workflows = client.list_workflows().await?;
        for name in &workflow_names {
            if let Some(wf) = workflows.items.iter().find(|w| &w.name == name) {
                client.delete_workflow(&wf.id).await?;
                if !json {
                    println!("  {} workflow '{}'", "deleted".red(), name);
                }
            } else if !json {
                println!("  {} workflow '{}' (not found)", "skipped".yellow(), name);
            }
        }
    }

    // Then delete agents
    if !agent_names.is_empty() {
        if !json {
            println!("{}", "Deleting agents...".blue().bold());
        }
        for name in &agent_names {
            if let Some(agent) = find_agent_by_name(client, name).await? {
                client.terminate_agent(&agent.id).await?;
                if !json {
                    println!("  {} agent '{}'", "deleted".red(), name);
                }
            } else if !json {
                println!("  {} agent '{}' (not found)", "skipped".yellow(), name);
            }
        }
    }

    if !json {
        println!();
        println!("{}", "Teardown complete.".green().bold());
    }

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_template() {
        let yaml = r#"
name: worker
working_dir: /tmp/project
shell: bash
interactive: false
worktree: true
prompt: "Fix all the bugs"
tool_policy:
  mode: allow_list
  tools: [Read, Grep, Edit]
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "worker");
        assert_eq!(tmpl.working_dir, "/tmp/project");
        assert_eq!(tmpl.shell, "bash");
        assert!(tmpl.worktree);
        assert_eq!(tmpl.prompt.unwrap(), "Fix all the bugs");
        assert_eq!(
            tmpl.tool_policy,
            ToolPolicy::AllowList { tools: vec!["Read".into(), "Grep".into(), "Edit".into()] }
        );
    }

    #[test]
    fn test_parse_agent_defaults() {
        let yaml = r#"
name: minimal
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "minimal");
        assert_eq!(tmpl.working_dir, ".");
        assert_eq!(tmpl.shell, "zsh");
        assert!(!tmpl.interactive);
        assert!(!tmpl.worktree);
        assert_eq!(tmpl.tool_policy, ToolPolicy::AllowAll);
    }

    #[test]
    fn test_parse_workflow_template() {
        let yaml = r#"
name: issue-worker
agent: worker
source:
  type: github_issues
  owner: org
  repo: repo
  labels: [bug, agent]
poll_interval: 120
prompt_template: "Fix: {{title}}\n{{body}}"
"#;
        let tmpl: WorkflowTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "issue-worker");
        assert_eq!(tmpl.agent, "worker");
        assert_eq!(tmpl.poll_interval, 120);
        assert!(tmpl.enabled);
    }

    #[test]
    fn test_parse_workflow_with_tool_policy() {
        let yaml = r#"
name: safe
agent: reviewer
source:
  type: github_issues
  owner: org
  repo: repo
tool_policy:
  mode: deny_all
prompt_template: "Review: {{title}}"
"#;
        let tmpl: WorkflowTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.tool_policy, Some(ToolPolicy::DenyAll));
    }

    #[test]
    fn test_parse_source_template() {
        let yaml = r#"
type: github_issues
owner: myorg
repo: myrepo
labels: [bug]
state: closed
"#;
        let src: SourceTemplate = serde_yaml::from_str(yaml).unwrap();
        match src {
            SourceTemplate::GithubIssues { owner, repo, labels, state } => {
                assert_eq!(owner, "myorg");
                assert_eq!(repo, "myrepo");
                assert_eq!(labels, vec!["bug"]);
                assert_eq!(state, "closed");
            }
        }
    }
}
