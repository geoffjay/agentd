//! Project management command implementations.
//!
//! This module implements all subcommands for managing projects via the
//! orchestrator service. Projects group agents and workflows together for
//! organisational purposes.
//!
//! # Available Commands
//!
//! - **list**: List all projects
//! - **create**: Create a new project
//! - **show**: Show details of a specific project (by name or UUID)
//! - **update**: Update a project's name or description
//! - **delete**: Delete a project
//! - **add-agent**: Associate an agent with a project
//! - **remove-agent**: Remove an agent from a project
//! - **add-workflow**: Associate a workflow with a project
//! - **remove-workflow**: Remove a workflow from a project
//!
//! # Examples
//!
//! ```bash
//! agent project list
//! agent project create my-project --description "Work items"
//! agent project show my-project
//! agent project add-agent my-project <agent-id>
//! agent project remove-agent my-project <agent-id>
//! ```

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::*;
use uuid::Uuid;

use orchestrator::client::OrchestratorClient;
use orchestrator::types::{CreateProjectRequest, UpdateProjectRequest};

/// Project management subcommands.
#[derive(Subcommand)]
pub enum ProjectCommand {
    /// List all projects.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project list
    /// ```
    List,

    /// Create a new project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project create my-project
    /// agent project create my-project --description "Description of the project"
    /// ```
    Create {
        /// Project name (must be unique)
        name: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,
    },

    /// Show details of a project by name or UUID.
    ///
    /// Displays the project details including associated agent and workflow counts.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project show my-project
    /// agent project show 550e8400-e29b-41d4-a716-446655440000
    /// ```
    Show {
        /// Project name or UUID
        id_or_name: String,
    },

    /// Update a project's name or description.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project update my-project --name new-name
    /// agent project update my-project --description "Updated description"
    /// ```
    Update {
        /// Project name or UUID
        id_or_name: String,

        /// New name for the project
        #[arg(long)]
        name: Option<String>,

        /// New description for the project
        #[arg(long)]
        description: Option<String>,
    },

    /// Delete a project.
    ///
    /// Fails if agents or workflows are still associated with the project.
    /// Dissociate them first with `remove-agent` and `remove-workflow`.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project delete my-project
    /// ```
    Delete {
        /// Project name or UUID
        id_or_name: String,
    },

    /// Associate an agent with a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project add-agent my-project 550e8400-e29b-41d4-a716-446655440000
    /// ```
    AddAgent {
        /// Project name or UUID
        project: String,

        /// Agent UUID
        agent_id: String,
    },

    /// Remove an agent from a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project remove-agent my-project 550e8400-e29b-41d4-a716-446655440000
    /// ```
    RemoveAgent {
        /// Project name or UUID
        project: String,

        /// Agent UUID
        agent_id: String,
    },

    /// Associate a workflow with a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project add-workflow my-project 550e8400-e29b-41d4-a716-446655440000
    /// ```
    AddWorkflow {
        /// Project name or UUID
        project: String,

        /// Workflow UUID
        workflow_id: String,
    },

    /// Remove a workflow from a project.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent project remove-workflow my-project 550e8400-e29b-41d4-a716-446655440000
    /// ```
    RemoveWorkflow {
        /// Project name or UUID
        project: String,

        /// Workflow UUID
        workflow_id: String,
    },
}

impl ProjectCommand {
    /// Execute the project command.
    pub async fn execute(&self, client: &OrchestratorClient, json: bool) -> Result<()> {
        match self {
            ProjectCommand::List => list_projects(client, json).await,
            ProjectCommand::Create { name, description } => {
                create_project(client, name, description.as_deref(), json).await
            }
            ProjectCommand::Show { id_or_name } => show_project(client, id_or_name, json).await,
            ProjectCommand::Update { id_or_name, name, description } => {
                update_project(client, id_or_name, name.as_deref(), description.as_deref(), json)
                    .await
            }
            ProjectCommand::Delete { id_or_name } => delete_project(client, id_or_name, json).await,
            ProjectCommand::AddAgent { project, agent_id } => {
                add_agent(client, project, agent_id, json).await
            }
            ProjectCommand::RemoveAgent { project, agent_id } => {
                remove_agent(client, project, agent_id, json).await
            }
            ProjectCommand::AddWorkflow { project, workflow_id } => {
                add_workflow(client, project, workflow_id, json).await
            }
            ProjectCommand::RemoveWorkflow { project, workflow_id } => {
                remove_workflow(client, project, workflow_id, json).await
            }
        }
    }
}

/// Resolve a project name or UUID string to a project UUID.
///
/// If the string parses as a UUID it is used directly; otherwise a name lookup
/// is performed against the orchestrator.
async fn resolve_project_id(client: &OrchestratorClient, id_or_name: &str) -> Result<Uuid> {
    if let Ok(id) = Uuid::parse_str(id_or_name) {
        return Ok(id);
    }
    match client.get_project_by_name(id_or_name).await? {
        Some(p) => Ok(p.id),
        None => bail!("No project found with name {:?}", id_or_name),
    }
}

async fn list_projects(client: &OrchestratorClient, json: bool) -> Result<()> {
    let resp = client.list_projects().await.context("Failed to list projects")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp.items)?);
        return Ok(());
    }

    if resp.items.is_empty() {
        println!("{}", "No projects found.".yellow());
        return Ok(());
    }

    println!("{}", "Projects".blue().bold());
    println!("{}", "=".repeat(60).cyan());
    for project in &resp.items {
        let desc = project.description.as_deref().unwrap_or("-");
        println!(
            "  {} | {} | {}",
            project.id.to_string().bright_black(),
            project.name.bold(),
            desc
        );
    }
    println!();
    println!("Total: {}", resp.total.to_string().green().bold());

    Ok(())
}

async fn create_project(
    client: &OrchestratorClient,
    name: &str,
    description: Option<&str>,
    json: bool,
) -> Result<()> {
    let req = CreateProjectRequest {
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
    };
    let project = client.create_project(&req).await.context("Failed to create project")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&project)?);
        return Ok(());
    }

    println!("{}", "Project created".green().bold());
    println!("  ID:          {}", project.id.to_string().bright_black());
    println!("  Name:        {}", project.name.bold());
    if let Some(desc) = &project.description {
        println!("  Description: {}", desc);
    }

    Ok(())
}

async fn show_project(client: &OrchestratorClient, id_or_name: &str, json: bool) -> Result<()> {
    let id = resolve_project_id(client, id_or_name).await?;
    let project = client.get_project(&id).await.context("Failed to get project")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&project)?);
        return Ok(());
    }

    println!("{}", "Project".blue().bold());
    println!("{}", "=".repeat(60).cyan());
    println!("  ID:          {}", project.id.to_string().bright_black());
    println!("  Name:        {}", project.name.bold());
    if let Some(desc) = &project.description {
        println!("  Description: {}", desc);
    }
    println!("  Created:     {}", project.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("  Updated:     {}", project.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));

    Ok(())
}

async fn update_project(
    client: &OrchestratorClient,
    id_or_name: &str,
    name: Option<&str>,
    description: Option<&str>,
    json: bool,
) -> Result<()> {
    if name.is_none() && description.is_none() {
        bail!("Provide at least one of --name or --description to update");
    }

    let id = resolve_project_id(client, id_or_name).await?;
    let req = UpdateProjectRequest {
        name: name.map(|s| s.to_string()),
        description: description.map(|s| s.to_string()),
    };
    let project = client.update_project(&id, &req).await.context("Failed to update project")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&project)?);
        return Ok(());
    }

    println!("{}", "Project updated".green().bold());
    println!("  ID:          {}", project.id.to_string().bright_black());
    println!("  Name:        {}", project.name.bold());
    if let Some(desc) = &project.description {
        println!("  Description: {}", desc);
    }

    Ok(())
}

async fn delete_project(client: &OrchestratorClient, id_or_name: &str, json: bool) -> Result<()> {
    let id = resolve_project_id(client, id_or_name).await?;
    client.delete_project(&id).await.context("Failed to delete project")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"deleted": id}))?);
        return Ok(());
    }

    println!("{} {}", "Deleted project".green().bold(), id.to_string().bright_black());

    Ok(())
}

async fn add_agent(
    client: &OrchestratorClient,
    project: &str,
    agent_id: &str,
    json: bool,
) -> Result<()> {
    let project_id = resolve_project_id(client, project).await?;
    let agent_uuid = Uuid::parse_str(agent_id).context("Invalid agent UUID")?;
    client
        .associate_project_agent(&project_id, &agent_uuid)
        .await
        .context("Failed to add agent to project")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"project_id": project_id, "agent_id": agent_uuid})
            )?
        );
        return Ok(());
    }

    println!(
        "{} agent {} to project {}",
        "Added".green().bold(),
        agent_uuid.to_string().bright_black(),
        project_id.to_string().bright_black()
    );

    Ok(())
}

async fn remove_agent(
    client: &OrchestratorClient,
    project: &str,
    agent_id: &str,
    json: bool,
) -> Result<()> {
    let project_id = resolve_project_id(client, project).await?;
    let agent_uuid = Uuid::parse_str(agent_id).context("Invalid agent UUID")?;
    client
        .dissociate_project_agent(&project_id, &agent_uuid)
        .await
        .context("Failed to remove agent from project")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"project_id": project_id, "agent_id": agent_uuid})
            )?
        );
        return Ok(());
    }

    println!(
        "{} agent {} from project {}",
        "Removed".green().bold(),
        agent_uuid.to_string().bright_black(),
        project_id.to_string().bright_black()
    );

    Ok(())
}

async fn add_workflow(
    client: &OrchestratorClient,
    project: &str,
    workflow_id: &str,
    json: bool,
) -> Result<()> {
    let project_id = resolve_project_id(client, project).await?;
    let workflow_uuid = Uuid::parse_str(workflow_id).context("Invalid workflow UUID")?;
    client
        .associate_project_workflow(&project_id, &workflow_uuid)
        .await
        .context("Failed to add workflow to project")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"project_id": project_id, "workflow_id": workflow_uuid})
            )?
        );
        return Ok(());
    }

    println!(
        "{} workflow {} to project {}",
        "Added".green().bold(),
        workflow_uuid.to_string().bright_black(),
        project_id.to_string().bright_black()
    );

    Ok(())
}

async fn remove_workflow(
    client: &OrchestratorClient,
    project: &str,
    workflow_id: &str,
    json: bool,
) -> Result<()> {
    let project_id = resolve_project_id(client, project).await?;
    let workflow_uuid = Uuid::parse_str(workflow_id).context("Invalid workflow UUID")?;
    client
        .dissociate_project_workflow(&project_id, &workflow_uuid)
        .await
        .context("Failed to remove workflow from project")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"project_id": project_id, "workflow_id": workflow_uuid})
            )?
        );
        return Ok(());
    }

    println!(
        "{} workflow {} from project {}",
        "Removed".green().bold(),
        workflow_uuid.to_string().bright_black(),
        project_id.to_string().bright_black()
    );

    Ok(())
}
