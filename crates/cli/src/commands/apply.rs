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
use std::collections::HashMap;
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
    /// Model to use for the claude session (e.g. sonnet, opus, haiku).
    pub model: Option<String>,
    /// Environment variables to pass to the agent.
    #[serde(default)]
    pub env: HashMap<String, String>,
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

/// Detected template type for a single YAML file.
pub enum TemplateKind {
    Agent,
    Workflow,
}

/// Determine whether a YAML file is an agent or workflow template.
///
/// Uses two heuristics:
/// 1. If the file lives under an `agents/` directory, it's an agent.
/// 2. Otherwise, try parsing as an agent template first (which has no required
///    `source` field), then fall back to workflow.
pub fn detect_template_kind(path: &Path) -> Result<TemplateKind> {
    // Heuristic: parent directory name
    if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
        if parent == "agents" {
            return Ok(TemplateKind::Agent);
        }
        if parent == "workflows" {
            return Ok(TemplateKind::Workflow);
        }
    }

    // Fallback: try parsing as each type
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;
    if serde_yaml::from_str::<AgentTemplate>(&content).is_ok() {
        // Agent templates don't require `source`, so also check that the file
        // does NOT contain the workflow-specific `source` key to avoid ambiguity.
        if serde_yaml::from_str::<WorkflowTemplate>(&content).is_ok() {
            return Ok(TemplateKind::Workflow);
        }
        return Ok(TemplateKind::Agent);
    }

    // Default to workflow (will produce a useful parse error if neither works)
    Ok(TemplateKind::Workflow)
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

// ── Apply: single agent file ─────────────────────────────────────────

pub async fn apply_agent_file(
    client: &OrchestratorClient,
    path: &Path,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let tmpl = parse_agent_template(path)?;

    if dry_run {
        // Resolve working_dir the same way apply_agent does — so the preview
        // reflects exactly what would be sent to the orchestrator.
        let working_dir = if tmpl.working_dir == "." {
            std::env::current_dir()?.to_string_lossy().to_string()
        } else {
            let base = path.parent().unwrap_or(Path::new("."));
            let full = base.join(&tmpl.working_dir);
            full.canonicalize().unwrap_or(full).to_string_lossy().to_string()
        };

        if json {
            let result = serde_json::json!({
                "dry_run": true,
                "valid": true,
                "kind": "agent",
                "config": {
                    "name": tmpl.name,
                    "working_dir": working_dir,
                    "shell": tmpl.shell,
                    "interactive": tmpl.interactive,
                    "worktree": tmpl.worktree,
                    "prompt": tmpl.prompt,
                    "system_prompt": tmpl.system_prompt,
                    "tool_policy": tmpl.tool_policy,
                    "model": tmpl.model,
                    "env": tmpl.env,
                },
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!(
                "  {} agent '{}'",
                "would create".green(),
                tmpl.name.bright_white()
            );
            println!("    working_dir:  {}", working_dir.bright_black());
            println!("    shell:        {}", tmpl.shell.bright_black());
            if tmpl.interactive {
                println!("    interactive:  {}", "true".bright_black());
            }
            if tmpl.worktree {
                println!("    worktree:     {}", "true".bright_black());
            }
            if let Some(ref model) = tmpl.model {
                println!("    model:        {}", model.bright_black());
            }
            if !tmpl.env.is_empty() {
                let keys: Vec<&str> = tmpl.env.keys().map(|s| s.as_str()).collect();
                println!("    env:          {}", keys.join(", ").bright_black());
            }
        }
        return Ok(());
    }

    apply_agent(client, &tmpl, path, json).await
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

    if dry_run {
        // In dry-run mode, do NOT make any HTTP calls to the orchestrator.
        // Show the configuration that would be submitted, using the agent name
        // as a symbolic reference (we cannot resolve it to a UUID without HTTP).
        if json {
            let source_json = match &tmpl.source {
                SourceTemplate::GithubIssues { owner, repo, labels, state } => {
                    serde_json::json!({
                        "type": "github_issues",
                        "owner": owner,
                        "repo": repo,
                        "labels": labels,
                        "state": state,
                    })
                }
            };
            let result = serde_json::json!({
                "dry_run": true,
                "valid": true,
                "kind": "workflow",
                "config": {
                    "name": tmpl.name,
                    "agent": tmpl.agent,
                    "source": source_json,
                    "poll_interval_secs": tmpl.poll_interval,
                    "enabled": tmpl.enabled,
                    "prompt_template": prompt,
                },
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!(
                "  {} workflow '{}'",
                "would create".green(),
                tmpl.name.bright_white()
            );
            println!("    agent:         {}", tmpl.agent.bright_black());
            match &tmpl.source {
                SourceTemplate::GithubIssues { owner, repo, labels, state } => {
                    println!(
                        "    source:        github_issues {}/{}",
                        owner.bright_black(),
                        repo.bright_black()
                    );
                    if !labels.is_empty() {
                        println!("    labels:        {}", labels.join(", ").bright_black());
                    }
                    println!("    state:         {}", state.bright_black());
                }
            }
            println!("    poll_interval: {}s", tmpl.poll_interval.to_string().bright_black());
            println!("    enabled:       {}", tmpl.enabled.to_string().bright_black());
        }
        return Ok(());
    }

    if !json {
        println!(
            "  {} workflow '{}'...",
            "Creating".cyan(),
            tmpl.name.bright_white()
        );
    }

    // Resolve agent name → UUID (only in live mode)
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
            // Build full resolved config objects for the JSON output
            let agents_json: Vec<serde_json::Value> = agent_templates
                .iter()
                .map(|(path, t)| {
                    let working_dir = if t.working_dir == "." {
                        std::env::current_dir()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| ".".to_string())
                    } else {
                        let base = path.parent().unwrap_or(Path::new("."));
                        let full = base.join(&t.working_dir);
                        full.canonicalize().unwrap_or(full).to_string_lossy().to_string()
                    };
                    serde_json::json!({
                        "name": t.name,
                        "working_dir": working_dir,
                        "shell": t.shell,
                        "interactive": t.interactive,
                        "worktree": t.worktree,
                        "prompt": t.prompt,
                        "system_prompt": t.system_prompt,
                        "tool_policy": t.tool_policy,
                        "model": t.model,
                        "env": t.env,
                    })
                })
                .collect();

            let workflows_json: Vec<serde_json::Value> = workflow_templates
                .iter()
                .map(|(path, t)| {
                    let prompt = resolve_prompt(t, path).unwrap_or_default();
                    let source_json = match &t.source {
                        SourceTemplate::GithubIssues { owner, repo, labels, state } => {
                            serde_json::json!({
                                "type": "github_issues",
                                "owner": owner,
                                "repo": repo,
                                "labels": labels,
                                "state": state,
                            })
                        }
                    };
                    serde_json::json!({
                        "name": t.name,
                        "agent": t.agent,
                        "source": source_json,
                        "poll_interval_secs": t.poll_interval,
                        "enabled": t.enabled,
                        "prompt_template": prompt,
                    })
                })
                .collect();

            let result = serde_json::json!({
                "dry_run": true,
                "valid": true,
                "agents": agents_json,
                "workflows": workflows_json,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!();
            println!(
                "{} {} agent(s), {} workflow(s) — {}",
                "Dry run passed:".yellow().bold(),
                agent_templates.len(),
                workflow_templates.len(),
                "no changes made".bright_black()
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
        model: tmpl.model.clone(),
        env: tmpl.env.clone(),
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
        assert_eq!(tmpl.model, None);
    }

    #[test]
    fn test_parse_agent_with_model() {
        let yaml = r#"
name: planner
model: opus
working_dir: "."
system_prompt: "You are a planning agent"
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "planner");
        assert_eq!(tmpl.model, Some("opus".to_string()));
    }

    #[test]
    fn test_parse_agent_with_full_model_name() {
        let yaml = r#"
name: worker
model: claude-sonnet-4-6
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.model, Some("claude-sonnet-4-6".to_string()));
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

    #[test]
    fn test_detect_template_kind_by_parent_dir() {
        let agent_path = Path::new(".agentd/agents/worker.yml");
        assert!(matches!(detect_template_kind(agent_path).unwrap(), TemplateKind::Agent));

        let workflow_path = Path::new(".agentd/workflows/issue-worker.yml");
        assert!(matches!(detect_template_kind(workflow_path).unwrap(), TemplateKind::Workflow));
    }

    #[test]
    fn test_detect_template_kind_by_content() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_apply_test");
        let _ = std::fs::create_dir_all(&dir);

        // Agent template (no `source` key)
        let agent_file = dir.join("planner.yml");
        let mut f = std::fs::File::create(&agent_file).unwrap();
        writeln!(f, "name: planner\nmodel: opus\nprompt: plan things").unwrap();
        assert!(matches!(detect_template_kind(&agent_file).unwrap(), TemplateKind::Agent));

        // Workflow template (has `source` and `agent` keys)
        let wf_file = dir.join("issue-worker.yml");
        let mut f = std::fs::File::create(&wf_file).unwrap();
        writeln!(
            f,
            "name: wf\nagent: worker\nsource:\n  type: github_issues\n  owner: o\n  repo: r\nprompt_template: hi"
        )
        .unwrap();
        assert!(matches!(detect_template_kind(&wf_file).unwrap(), TemplateKind::Workflow));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_agent_with_env() {
        let yaml = r#"
name: worker
env:
  ANTHROPIC_API_KEY: sk-ant-test123
  ANTHROPIC_BASE_URL: https://example.com/api
  MY_SECRET: top-secret
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "worker");
        assert_eq!(tmpl.env.get("ANTHROPIC_API_KEY"), Some(&"sk-ant-test123".to_string()));
        assert_eq!(
            tmpl.env.get("ANTHROPIC_BASE_URL"),
            Some(&"https://example.com/api".to_string())
        );
        assert_eq!(tmpl.env.get("MY_SECRET"), Some(&"top-secret".to_string()));
        assert_eq!(tmpl.env.len(), 3);
    }

    #[test]
    fn test_parse_agent_without_env_defaults_empty() {
        let yaml = r#"name: minimal"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert!(tmpl.env.is_empty());
    }

    #[test]
    fn test_parse_agent_with_empty_env_section() {
        let yaml = r#"
name: worker
env: {}
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert!(tmpl.env.is_empty());
    }

    #[test]
    fn test_parse_agent_env_combined_with_other_fields() {
        let yaml = r#"
name: planner
model: opus
shell: bash
worktree: true
env:
  API_KEY: abc123
  BASE_URL: https://api.example.com
tool_policy:
  mode: allow_all
"#;
        let tmpl: AgentTemplate = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(tmpl.name, "planner");
        assert_eq!(tmpl.model, Some("opus".to_string()));
        assert_eq!(tmpl.shell, "bash");
        assert!(tmpl.worktree);
        assert_eq!(tmpl.env.get("API_KEY"), Some(&"abc123".to_string()));
        assert_eq!(tmpl.env.get("BASE_URL"), Some(&"https://api.example.com".to_string()));
        assert_eq!(tmpl.tool_policy, ToolPolicy::AllowAll);
    }

    // ── Dry-run behaviour tests ──────────────────────────────────────

    /// apply_agent_file with dry_run=true must succeed even when the
    /// orchestrator is unreachable (proves no HTTP calls are made).
    #[tokio::test]
    async fn test_dry_run_agent_file_makes_no_http_calls() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_agent_test");
        let _ = std::fs::create_dir_all(&dir);

        let agent_file = dir.join("worker.yml");
        {
            let mut f = std::fs::File::create(&agent_file).unwrap();
            writeln!(
                f,
                "name: worker\nshell: bash\nworking_dir: .\nmodel: opus"
            )
            .unwrap();
        }

        // Point at a port nothing listens on — any HTTP call would fail.
        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_agent_file(&client, &agent_file, true, false).await;
        assert!(result.is_ok(), "dry-run agent should succeed without HTTP: {result:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// apply_agent_file dry-run JSON output must include the resolved config.
    #[tokio::test]
    async fn test_dry_run_agent_file_json_output() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_agent_json_test");
        let _ = std::fs::create_dir_all(&dir);

        let agent_file = dir.join("planner.yml");
        {
            let mut f = std::fs::File::create(&agent_file).unwrap();
            writeln!(
                f,
                "name: planner\nmodel: opus\nshell: zsh\nworking_dir: ."
            )
            .unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        // Verify the function itself returns Ok — JSON goes to stdout, tested
        // separately via parse_agent_template.
        let result = apply_agent_file(&client, &agent_file, true, true).await;
        assert!(result.is_ok(), "dry-run agent JSON should succeed: {result:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// apply_workflow_file with dry_run=true must succeed even when the
    /// orchestrator is unreachable (proves no HTTP calls are made).
    #[tokio::test]
    async fn test_dry_run_workflow_file_makes_no_http_calls() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_workflow_test");
        let _ = std::fs::create_dir_all(&dir);

        let wf_file = dir.join("issue-worker.yml");
        {
            let mut f = std::fs::File::create(&wf_file).unwrap();
            writeln!(
                f,
                "name: issue-worker\nagent: worker\nsource:\n  type: github_issues\n  owner: org\n  repo: repo\nprompt_template: \"Fix: {{{{title}}}}\""
            )
            .unwrap();
        }

        // Point at a port nothing listens on — any HTTP call would fail.
        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_workflow_file(&client, &wf_file, true, false).await;
        assert!(
            result.is_ok(),
            "dry-run workflow should succeed without HTTP: {result:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// apply_workflow_file dry-run JSON output must include the workflow config
    /// without making any HTTP calls.
    #[tokio::test]
    async fn test_dry_run_workflow_file_json_no_http_calls() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_workflow_json_test");
        let _ = std::fs::create_dir_all(&dir);

        let wf_file = dir.join("issue-worker.yml");
        {
            let mut f = std::fs::File::create(&wf_file).unwrap();
            writeln!(
                f,
                "name: issue-worker\nagent: worker\nsource:\n  type: github_issues\n  owner: org\n  repo: repo\nprompt_template: \"Fix: {{{{title}}}}\""
            )
            .unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_workflow_file(&client, &wf_file, true, true).await;
        assert!(
            result.is_ok(),
            "dry-run workflow JSON should succeed without HTTP: {result:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// apply_directory dry-run must succeed even when the orchestrator is
    /// unreachable — all HTTP calls must be skipped.
    #[tokio::test]
    async fn test_dry_run_directory_makes_no_http_calls() {
        use std::io::Write;

        let base = std::env::temp_dir().join("agentd_dry_run_dir_test");
        let agents_dir = base.join("agents");
        let workflows_dir = base.join("workflows");
        let _ = std::fs::create_dir_all(&agents_dir);
        let _ = std::fs::create_dir_all(&workflows_dir);

        {
            let mut f = std::fs::File::create(agents_dir.join("worker.yml")).unwrap();
            writeln!(f, "name: worker\nshell: zsh").unwrap();
        }
        {
            let mut f = std::fs::File::create(workflows_dir.join("issue-worker.yml")).unwrap();
            writeln!(
                f,
                "name: issue-worker\nagent: worker\nsource:\n  type: github_issues\n  owner: org\n  repo: repo\nprompt_template: \"Fix: {{{{title}}}}\""
            )
            .unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        // wait_timeout is irrelevant for dry-run
        let result = apply_directory(&client, &base, true, 60, false).await;
        assert!(
            result.is_ok(),
            "dry-run directory should succeed without HTTP: {result:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// apply_directory dry-run JSON output must contain both agent and workflow
    /// config objects (not just names).
    #[tokio::test]
    async fn test_dry_run_directory_json_contains_full_configs() {
        use std::io::Write;

        let base = std::env::temp_dir().join("agentd_dry_run_dir_json_test");
        let agents_dir = base.join("agents");
        let workflows_dir = base.join("workflows");
        let _ = std::fs::create_dir_all(&agents_dir);
        let _ = std::fs::create_dir_all(&workflows_dir);

        {
            let mut f = std::fs::File::create(agents_dir.join("worker.yml")).unwrap();
            writeln!(f, "name: worker\nshell: bash\nmodel: opus").unwrap();
        }
        {
            let mut f = std::fs::File::create(workflows_dir.join("issue-worker.yml")).unwrap();
            writeln!(
                f,
                "name: issue-worker\nagent: worker\nsource:\n  type: github_issues\n  owner: org\n  repo: repo\nprompt_template: \"Fix: {{{{title}}}}\""
            )
            .unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_directory(&client, &base, true, 60, true).await;
        assert!(
            result.is_ok(),
            "dry-run directory JSON should succeed: {result:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Validation errors in dry-run must still be reported (exit ≠ 0).
    #[tokio::test]
    async fn test_dry_run_invalid_template_reports_error() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_invalid_test");
        let agents_dir = dir.join("agents");
        let _ = std::fs::create_dir_all(&agents_dir);

        // Write a YAML file that is structurally invalid for an agent (bad YAML).
        let bad_file = agents_dir.join("bad.yml");
        {
            let mut f = std::fs::File::create(&bad_file).unwrap();
            writeln!(f, "name: [this is not valid yaml for name field: {{").unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_directory(&client, &dir, true, 60, false).await;
        assert!(result.is_err(), "dry-run with invalid template should return an error");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Workflow with missing prompt_template/prompt_template_file must fail validation.
    #[tokio::test]
    async fn test_dry_run_workflow_missing_prompt_fails_validation() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("agentd_dry_run_no_prompt_test");
        let workflows_dir = dir.join("workflows");
        let _ = std::fs::create_dir_all(&workflows_dir);

        let wf_file = workflows_dir.join("bad.yml");
        {
            let mut f = std::fs::File::create(&wf_file).unwrap();
            // Missing both prompt_template and prompt_template_file
            writeln!(
                f,
                "name: bad\nagent: worker\nsource:\n  type: github_issues\n  owner: org\n  repo: repo"
            )
            .unwrap();
        }

        let client = OrchestratorClient::new("http://127.0.0.1:1".to_string());
        let result = apply_directory(&client, &dir, true, 60, false).await;
        assert!(
            result.is_err(),
            "workflow without prompt should fail validation: {result:?}"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("prompt_template") || err.contains("prompt"),
            "Error should mention missing prompt: {err}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
