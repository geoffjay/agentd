//! Organization management command implementations.
//!
//! Provides CLI subcommands for managing agentd organizations via the core service.
//! All commands require a valid session token (stored by `agent auth login`).
//!
//! # Available Commands
//!
//! - **list**   — List all organizations the current user belongs to
//! - **create** — Create a new organization
//! - **switch** — Set the active organization by name or ID
//!
//! # Examples
//!
//! ```bash
//! agent org list
//! agent org create "Acme Corp" --slug acme-corp
//! agent org switch acme-corp
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use serde::{Deserialize, Serialize};

use super::auth::load_token;

// ---------------------------------------------------------------------------
// HTTP helpers (mirrors auth.rs but for org endpoints)
// ---------------------------------------------------------------------------

fn core_base_url() -> String {
    std::env::var("AGENTD_CORE_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:17000".to_string())
}

async fn get_json_auth<T: for<'de> Deserialize<'de>>(path: &str, token: &str) -> Result<T> {
    let url = format!("{}{}", core_base_url(), path);
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let msg = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {}", status, msg);
    }
    resp.json().await.context("deserializing response")
}

async fn post_json_auth<B: Serialize, T: for<'de> Deserialize<'de>>(
    path: &str,
    body: &B,
    token: &str,
) -> Result<T> {
    let url = format!("{}{}", core_base_url(), path);
    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let msg = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {}", status, msg);
    }
    resp.json().await.context("deserializing response")
}

async fn put_json_auth<B: Serialize, T: for<'de> Deserialize<'de>>(
    path: &str,
    body: &B,
    token: &str,
) -> Result<T> {
    let url = format!("{}{}", core_base_url(), path);
    let resp = reqwest::Client::new()
        .put(&url)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .with_context(|| format!("PUT {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let msg = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {}", status, msg);
    }
    resp.json().await.context("deserializing response")
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct OrgResponse {
    id: String,
    name: String,
    slug: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserProfileResponse {
    id: String,
    username: Option<String>,
    email: String,
    active_organization_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Command definition
// ---------------------------------------------------------------------------

/// Subcommands for organization management.
#[derive(Debug, Subcommand)]
pub enum OrgCommand {
    /// List all organizations you belong to.
    ///
    /// Requires an active session (run `agent auth login` first).
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent org list
    /// agent org list --json
    /// ```
    List,

    /// Create a new organization.
    ///
    /// You become the owner of the new organization. The `--slug` flag is
    /// optional — if omitted it is derived from `name` by lowercasing and
    /// replacing spaces with hyphens.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent org create "Acme Corp"
    /// agent org create "Acme Corp" --slug acme-corp
    /// ```
    Create {
        /// Organization display name
        name: String,
        /// URL-safe slug (derived from name if omitted)
        #[arg(long)]
        slug: Option<String>,
    },

    /// Switch your active organization.
    ///
    /// Accepts an organization name or ID. The active organization is used
    /// by the API gateway to scope requests to the correct tenant.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent org switch acme-corp
    /// agent org switch "Acme Corp"
    /// ```
    Switch {
        /// Organization name or ID to switch to
        name_or_id: String,
    },
}

impl OrgCommand {
    pub async fn execute(&self, json: bool) -> Result<()> {
        match self {
            OrgCommand::List => list_cmd(json).await,
            OrgCommand::Create { name, slug } => create_cmd(name, slug.as_deref(), json).await,
            OrgCommand::Switch { name_or_id } => switch_cmd(name_or_id, json).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_cmd(json: bool) -> Result<()> {
    let token = load_token().context("not logged in — run `agent auth login` first")?;
    let orgs: Vec<OrgResponse> = get_json_auth("/api/v1/users/me/organizations", &token).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&orgs).unwrap_or_default());
        return Ok(());
    }

    if orgs.is_empty() {
        println!("{}", "No organizations found.".yellow());
        return Ok(());
    }

    println!("{}", "Organizations".blue().bold());
    println!("{}", "─".repeat(50).cyan());
    for org in &orgs {
        println!("  {} {}", org.name.cyan().bold(), format!("({})", org.slug).bright_black());
        println!("    ID: {}", org.id.bright_black());
    }
    println!("\n  {} organizations", orgs.len().to_string().green().bold());
    Ok(())
}

async fn create_cmd(name: &str, slug: Option<&str>, json: bool) -> Result<()> {
    let token = load_token().context("not logged in — run `agent auth login` first")?;

    // Derive slug from name if not provided
    let derived_slug: String;
    let slug = match slug {
        Some(s) => s,
        None => {
            derived_slug = name
                .chars()
                .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string();
            &derived_slug
        }
    };

    #[derive(Serialize)]
    struct CreateOrgReq<'a> {
        name: &'a str,
        slug: &'a str,
    }

    let org: OrgResponse =
        post_json_auth("/api/v1/organizations", &CreateOrgReq { name, slug }, &token).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&org).unwrap_or_default());
        return Ok(());
    }

    println!("{}", "✅ Organization created".green().bold());
    println!("  {}  {}", "Name:".bright_black(), org.name.cyan().bold());
    println!("  {}  {}", "Slug:".bright_black(), org.slug);
    println!("  {}  {}", "ID:".bright_black(), org.id.bright_black());
    Ok(())
}

async fn switch_cmd(name_or_id: &str, json: bool) -> Result<()> {
    let token = load_token().context("not logged in — run `agent auth login` first")?;

    // Fetch all orgs the user belongs to, then find by name or ID
    let orgs: Vec<OrgResponse> = get_json_auth("/api/v1/users/me/organizations", &token).await?;

    let target = orgs
        .iter()
        .find(|o| o.id == name_or_id || o.name == name_or_id || o.slug == name_or_id)
        .with_context(|| {
            format!(
                "organization '{}' not found — use `agent org list` to see your organizations",
                name_or_id
            )
        })?;

    #[derive(Serialize)]
    struct SwitchReq<'a> {
        organization_id: &'a str,
    }

    let updated: UserProfileResponse = put_json_auth(
        "/users/me/active-organization",
        &SwitchReq { organization_id: &target.id },
        &token,
    )
    .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&updated).unwrap_or_default());
        return Ok(());
    }

    println!("{} Switched to {}", "✅".green(), target.name.cyan().bold());
    println!("  {}  {}", "Slug:".bright_black(), target.slug);
    println!("  {}  {}", "ID:".bright_black(), target.id.bright_black());
    Ok(())
}
