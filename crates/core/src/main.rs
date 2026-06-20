//! Core service entry point.
//!
//! Central authentication and API gateway service for agentd.
//!
//! # Subcommands
//!
//! ```text
//! agentd-core                     # default: start HTTP server
//! agentd-core serve               # start HTTP server (explicit)
//! agentd-core migrate status      # show migration status
//! agentd-core migrate up          # apply all pending migrations
//! agentd-core migrate down        # roll back latest migration (requires --yes or TTY)
//! agentd-core migrate down --all  # roll back all migrations (requires --yes or TTY)
//! agentd-core set-superuser <email>          # grant product-admin (superuser) access
//! agentd-core set-superuser <email> --unset  # revoke superuser access
//! agentd-core set-password <email>           # reset a user's password (prompts on stdin)
//! ```
//!
//! # Environment Variables
//!
//! | Variable            | Default | Description            |
//! |---------------------|---------|------------------------|
//! | `AGENTD_PORT`       | `17000` | HTTP listen port (dev default; production uses 7000) |
//! | `RUST_LOG`          | `info`  | Log level / filter     |
//! | `AGENTD_LOG_FORMAT` | (text)  | Set to `json` for JSON |
//! | `AGENTD_ENV`        | (prod)  | `development`/`test`   |

use std::io::IsTerminal;
use std::path::PathBuf;

use agentd_common::config::ValidateConfig;
use anyhow::Result;
use axum::{extract::State, response::IntoResponse, routing::get};
use clap::{Parser, Subcommand};
use metrics_exporter_prometheus::PrometheusHandle;
use std::future::IntoFuture;
use tracing::info;

// ---------------------------------------------------------------------------
// CLI definitions
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "agentd-core",
    about = "Core authentication and API gateway service for agentd",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP server (default when no subcommand is given)
    Serve,
    /// Manage database migrations
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
    /// Grant or revoke product-level superuser (product-admin) access for a user
    SetSuperuser {
        /// Email address of the registered user to modify
        email: String,
        /// Revoke superuser access instead of granting it
        #[arg(long)]
        unset: bool,
    },
    /// Reset a user's password (prompts for the new password on stdin)
    SetPassword {
        /// Email address of the registered user to modify
        email: String,
    },
}

#[derive(Subcommand)]
enum MigrateAction {
    /// Show the applied / pending state of all migrations
    Status {
        /// Path to the SQLite database file (overrides AGENTD_ENV-based resolution)
        #[arg(long)]
        db_path: Option<PathBuf>,
    },
    /// Apply all pending migrations
    Up {
        /// Path to the SQLite database file (overrides AGENTD_ENV-based resolution)
        #[arg(long)]
        db_path: Option<PathBuf>,
    },
    /// Roll back migrations (requires --yes or interactive confirmation on a TTY)
    Down {
        /// Roll back ALL applied migrations instead of just the latest one
        #[arg(long)]
        all: bool,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
        /// Path to the SQLite database file (overrides AGENTD_ENV-based resolution)
        #[arg(long)]
        db_path: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Server helpers
// ---------------------------------------------------------------------------

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info",
        "version" => env!("CARGO_PKG_VERSION"),
        "service" => "core"
    )
    .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    info!("Shutdown signal received");
}

async fn run_serve() -> Result<()> {
    info!("Starting agentd-core service...");

    let db_path = agentd_common::storage::get_db_path("agentd-core", "core.db")?;
    let db = agentd_common::storage::create_connection(&db_path).await?;
    let storage = agentd_core::storage::Storage::new(db).await?;
    info!("Database migrations applied");

    let metrics_handle = init_metrics();

    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    let state = agentd_core::api::AppState::with_pam_loaded(storage);

    // Build the gateway's upstream map from the shared `[services.core]`
    // config (with bare `*_URL` env vars still overriding for compatibility).
    let shared = agentd_common::config::load().unwrap_or_else(|e| {
        tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
        agentd_common::config::AgentdConfig::default()
    });
    let proxy = agentd_core::proxy::ProxyConfig::from_config(&shared.services.core);

    let app = agentd_core::api::create_router_with_proxy(state, proxy)
        .merge(metrics_router)
        .layer(agentd_common::server::metrics_layer())
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let cfg = agentd_core::config::CoreConfig::load();
    cfg.validate()?;
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Core API listening on http://{}", addr);

    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).into_future().await?;

    info!("Core service shut down");
    Ok(())
}

// ---------------------------------------------------------------------------
// Migration helpers
// ---------------------------------------------------------------------------

/// Resolve the database path: use the explicit override if provided, otherwise
/// fall back to AGENTD_ENV-aware path resolution.
fn resolve_db_path(override_path: Option<&PathBuf>) -> Result<PathBuf> {
    match override_path {
        Some(p) => Ok(p.clone()),
        None => agentd_common::storage::get_db_path("agentd-core", "core.db"),
    }
}

async fn run_migrate_status(db_path_override: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(db_path_override.as_ref())?;
    let statuses = agentd_core::migration_status_for_path(&db_path).await?;

    println!("Migration Status (agentd-core):");
    let mut applied = 0usize;
    let mut pending = 0usize;
    for (name, is_applied) in &statuses {
        if *is_applied {
            println!("  \u{2713} applied  {name}");
            applied += 1;
        } else {
            println!("  \u{2717} pending  {name}");
            pending += 1;
        }
    }
    println!("  {applied} applied, {pending} pending");
    Ok(())
}

async fn run_migrate_up(db_path_override: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(db_path_override.as_ref())?;

    println!("Applying migrations...");
    agentd_core::apply_migrations_for_path(&db_path).await?;

    // Report final status
    let statuses = agentd_core::migration_status_for_path(&db_path).await?;
    let applied = statuses.iter().filter(|(_, ok)| *ok).count();
    let pending = statuses.iter().filter(|(_, ok)| !ok).count();

    if pending == 0 {
        println!("  \u{2713} up to date ({applied} applied, 0 pending)");
    } else {
        println!("  \u{2713} done ({applied} applied, {pending} still pending)");
    }
    Ok(())
}

async fn run_migrate_down(all: bool, yes: bool, db_path_override: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(db_path_override.as_ref())?;

    // Determine which migrations will be rolled back for the prompt message.
    let statuses = agentd_core::migration_status_for_path(&db_path).await?;
    let applied: Vec<&str> =
        statuses.iter().filter(|(_, ok)| *ok).map(|(n, _)| n.as_str()).collect();

    if applied.is_empty() {
        println!("Nothing to roll back — no migrations are applied.");
        return Ok(());
    }

    let targets: Vec<&str> = if all { applied.clone() } else { vec![applied[applied.len() - 1]] };

    // Require explicit confirmation when no TTY is attached.
    if !yes {
        let is_tty = std::io::stdin().is_terminal();
        if !is_tty {
            anyhow::bail!(
                "stdin is not a TTY — pass --yes to confirm rollback without a prompt.\n\
                 Would roll back: {}",
                targets.join(", ")
            );
        }

        // Interactive prompt
        println!("Will roll back the following migration(s):");
        for t in &targets {
            println!("  - {t}");
        }
        print!("Confirm? [y/N] ");
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let steps = if all { None } else { Some(1u32) };
    println!("Rolling back {}...", if all { "all migrations" } else { "latest migration" });
    agentd_core::rollback_migrations_for_path(&db_path, steps).await?;

    for t in &targets {
        println!("  \u{2713} rolled back {t}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Superuser bootstrap
// ---------------------------------------------------------------------------

/// Grant or revoke the product-level superuser flag for an existing user,
/// operating directly on the core database. Running against the DB (rather than
/// an authenticated endpoint) avoids the chicken-and-egg of needing a superuser
/// to create the first one.
async fn run_set_superuser(email: String, unset: bool) -> Result<()> {
    let db_path = agentd_common::storage::get_db_path("agentd-core", "core.db")?;
    let db = agentd_common::storage::create_connection(&db_path).await?;
    let storage = agentd_core::storage::Storage::new(db).await?;

    let user = storage
        .users()
        .get_by_email(&email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no registered user with email: {email}"))?;

    let updated = storage.users().set_superuser(&user.id, !unset).await?;
    if updated.is_superuser {
        println!("\u{2713} {} is now a superuser (product-admin access granted)", updated.email);
    } else {
        println!("\u{2713} {} is no longer a superuser", updated.email);
    }
    Ok(())
}

/// Reset an existing user's password, operating directly on the core database.
/// Prompts for the new password on stdin so it does not land in shell history.
async fn run_set_password(email: String) -> Result<()> {
    let db_path = agentd_common::storage::get_db_path("agentd-core", "core.db")?;
    let db = agentd_common::storage::create_connection(&db_path).await?;
    let storage = agentd_core::storage::Storage::new(db).await?;

    let user = storage
        .users()
        .get_by_email(&email)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no registered user with email: {email}"))?;

    print!("New password for {}: ", user.email);
    use std::io::Write;
    std::io::stdout().flush()?;
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim();
    if password.is_empty() {
        anyhow::bail!("password must not be empty");
    }

    storage.users().update_password(&user.id, password).await?;
    println!("\u{2713} password updated for {}", user.email);
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    agentd_common::server::init_tracing();

    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Serve) => run_serve().await,
        Some(Command::Migrate { action }) => match action {
            MigrateAction::Status { db_path } => run_migrate_status(db_path).await,
            MigrateAction::Up { db_path } => run_migrate_up(db_path).await,
            MigrateAction::Down { all, yes, db_path } => run_migrate_down(all, yes, db_path).await,
        },
        Some(Command::SetSuperuser { email, unset }) => run_set_superuser(email, unset).await,
        Some(Command::SetPassword { email }) => run_set_password(email).await,
    }
}
