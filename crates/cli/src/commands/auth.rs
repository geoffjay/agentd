//! Authentication command implementations.
//!
//! Provides CLI subcommands for interacting with the agentd-core authentication
//! endpoints. The session token is stored at `~/.config/agentd/session` with
//! file permissions restricted to 0600 (owner read/write only).
//!
//! # Available Commands
//!
//! - **register** — Create a new account (prompts for username, email, password)
//! - **login**    — Authenticate with username or email + password
//! - **logout**   — Invalidate the current session token
//! - **status**   — Show currently logged-in user and active organization
//!
//! # Examples
//!
//! ```bash
//! agent auth register
//! agent auth login
//! agent auth logout
//! agent auth status
//! ```

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use dialoguer::{Input, Password};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Session storage helpers
// ---------------------------------------------------------------------------

/// Returns the path to the session token file: `~/.config/agentd/session`.
pub fn session_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("cannot determine config directory")?;
    Ok(config_dir.join("agentd").join("session"))
}

/// Load the session token from disk. Returns `None` if not found.
pub fn load_token() -> Option<String> {
    let path = session_path().ok()?;
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Persist the session token to `~/.config/agentd/session` with 0600 permissions.
pub fn save_token(token: &str) -> Result<()> {
    let path = session_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config dir: {}", parent.display()))?;
    }
    fs::write(&path, token)
        .with_context(|| format!("failed to write session file: {}", path.display()))?;

    // Restrict permissions to owner read/write (0600) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on: {}", path.display()))?;
    }

    Ok(())
}

/// Remove the session token from disk.
pub fn clear_token() -> Result<()> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove session file: {}", path.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP client helpers
// ---------------------------------------------------------------------------

fn core_base_url() -> String {
    crate::client::core_url()
}

async fn post_json<B: Serialize, T: for<'de> Deserialize<'de>>(
    path: &str,
    body: &B,
    token: Option<&str>,
) -> Result<T> {
    let url = format!("{}{}", core_base_url(), path);
    let client = reqwest::Client::new();
    let mut req = client.post(&url).json(body);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let resp = req.send().await.with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let msg = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("HTTP {}: {}", status, msg);
    }
    resp.json().await.context("deserializing response")
}

async fn get_json<T: for<'de> Deserialize<'de>>(path: &str, token: &str) -> Result<T> {
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

// ---------------------------------------------------------------------------
// Response types (mirrors core service API)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct UserResponse {
    id: String,
    username: Option<String>,
    email: String,
    display_name: Option<String>,
    role: String,
    active_organization_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OrgResponse {
    id: String,
    name: String,
    slug: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct LoginResponse {
    token: String,
    user: UserResponse,
    active_organization: Option<OrgResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MeResponse {
    user: UserResponse,
    active_organization: Option<OrgResponse>,
}

// ---------------------------------------------------------------------------
// Command definition
// ---------------------------------------------------------------------------

/// Subcommands for authentication with the core service.
#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Create a new agentd account.
    ///
    /// Prompts for username, email, optional display name, and password
    /// (with confirmation). Creates a default personal organization and
    /// stores the session token for subsequent commands.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent auth register
    /// ```
    Register,

    /// Log in to agentd.
    ///
    /// Accepts username or email with password. Stores the session token
    /// at `~/.config/agentd/session` for use by other commands.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent auth login
    /// agent auth login --username alice
    /// agent auth login --email alice@example.com
    /// ```
    Login {
        /// Login by username (optional — prompted if neither flag is given)
        #[arg(long)]
        username: Option<String>,
        /// Login by email
        #[arg(long)]
        email: Option<String>,
    },

    /// Log out of agentd.
    ///
    /// Calls the logout endpoint to invalidate the session token on the
    /// server, then removes the local token file.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent auth logout
    /// ```
    Logout,

    /// Show current authentication status.
    ///
    /// Displays the logged-in user and active organization if a valid
    /// session token is found.
    ///
    /// # Examples
    ///
    /// ```bash
    /// agent auth status
    /// ```
    Status,
}

impl AuthCommand {
    pub async fn execute(&self, json: bool) -> Result<()> {
        match self {
            AuthCommand::Register => register_cmd(json).await,
            AuthCommand::Login { username, email } => {
                login_cmd(username.as_deref(), email.as_deref(), json).await
            }
            AuthCommand::Logout => logout_cmd(json).await,
            AuthCommand::Status => status_cmd(json).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn register_cmd(json: bool) -> Result<()> {
    let username: String =
        Input::new().with_prompt("Username").interact_text().context("reading username")?;
    let email: String =
        Input::new().with_prompt("Email").interact_text().context("reading email")?;
    let display_name: String = Input::new()
        .with_prompt("Display name (optional, press Enter to skip)")
        .allow_empty(true)
        .interact_text()
        .context("reading display name")?;
    let password = Password::new()
        .with_prompt("Password")
        .with_confirmation("Confirm password", "Passwords do not match")
        .interact()
        .context("reading password")?;

    #[derive(Serialize)]
    struct RegisterReq<'a> {
        username: &'a str,
        email: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_name: Option<&'a str>,
        password: &'a str,
    }

    let dn = if display_name.is_empty() { None } else { Some(display_name.as_str()) };
    let resp: LoginResponse = post_json(
        "/auth/register",
        &RegisterReq { username: &username, email: &email, display_name: dn, password: &password },
        None,
    )
    .await?;

    save_token(&resp.token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp.user).unwrap_or_default());
        return Ok(());
    }

    println!("{}", "✅ Account created and logged in".green().bold());
    print_user_info(&resp.user, resp.active_organization.as_ref());
    Ok(())
}

async fn login_cmd(username: Option<&str>, email: Option<&str>, json: bool) -> Result<()> {
    #[derive(Serialize)]
    struct LoginReq<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<&'a str>,
        password: &'a str,
    }

    // Determine identifier
    let (resolved_username, resolved_email) = if username.is_some() || email.is_some() {
        (username.map(str::to_string), email.map(str::to_string))
    } else {
        let input: String = Input::new()
            .with_prompt("Username or email")
            .interact_text()
            .context("reading identifier")?;
        if input.contains('@') {
            (None, Some(input))
        } else {
            (Some(input), None)
        }
    };

    let password =
        Password::new().with_prompt("Password").interact().context("reading password")?;

    let resp: LoginResponse = post_json(
        "/auth/login",
        &LoginReq {
            username: resolved_username.as_deref(),
            email: resolved_email.as_deref(),
            password: &password,
        },
        None,
    )
    .await?;

    save_token(&resp.token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp.user).unwrap_or_default());
        return Ok(());
    }

    println!("{}", "✅ Logged in".green().bold());
    print_user_info(&resp.user, resp.active_organization.as_ref());
    Ok(())
}

async fn logout_cmd(json: bool) -> Result<()> {
    let token = load_token().context("not logged in — no session token found")?;

    #[derive(Serialize)]
    struct Empty {}

    // Best-effort server-side invalidation
    let result: Result<serde_json::Value> =
        post_json("/auth/logout", &Empty {}, Some(&token)).await;
    if let Err(e) = result {
        eprintln!("{} server logout failed: {}", "⚠".yellow(), e);
    }

    clear_token()?;

    if json {
        println!("{}", serde_json::json!({ "status": "logged_out" }));
        return Ok(());
    }

    println!("{}", "✅ Logged out".green().bold());
    Ok(())
}

async fn status_cmd(json: bool) -> Result<()> {
    let token = match load_token() {
        Some(t) => t,
        None => {
            if json {
                println!("{}", serde_json::json!({ "status": "unauthenticated" }));
            } else {
                println!("{}", "Not logged in".yellow());
            }
            return Ok(());
        }
    };

    let resp: MeResponse =
        get_json("/auth/me", &token).await.context("failed to fetch session info")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "user": {
                    "id": resp.user.id,
                    "username": resp.user.username,
                    "email": resp.user.email,
                    "display_name": resp.user.display_name,
                    "role": resp.user.role,
                    "active_organization_id": resp.user.active_organization_id,
                },
                "active_organization": resp.active_organization.as_ref().map(|o| serde_json::json!({
                    "id": o.id,
                    "name": o.name,
                    "slug": o.slug,
                })),
            }))
            .unwrap_or_default()
        );
        return Ok(());
    }

    println!("{}", "Auth Status".blue().bold());
    println!("{}", "─".repeat(40).cyan());
    print_user_info(&resp.user, resp.active_organization.as_ref());
    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn print_user_info(user: &UserResponse, org: Option<&OrgResponse>) {
    println!(
        "  {}  {}",
        "User:".bright_black(),
        user.username.as_deref().unwrap_or(&user.email).cyan().bold()
    );
    println!("  {}  {}", "Email:".bright_black(), user.email);
    if let Some(dn) = &user.display_name {
        println!("  {}  {}", "Name:".bright_black(), dn);
    }
    println!("  {}  {}", "Role:".bright_black(), user.role.yellow());
    if let Some(o) = org {
        println!(
            "  {}  {} {}",
            "Active org:".bright_black(),
            o.name.green().bold(),
            format!("({})", o.slug).bright_black()
        );
    } else {
        println!("  {}  {}", "Active org:".bright_black(), "none".bright_black());
    }
}
