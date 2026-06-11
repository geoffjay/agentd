//! SeaORM migration runner shared by the CLI (`agent migrate`) and `xtask`.
//!
//! Each service owns its migrations and exposes `apply_migrations_for_path` /
//! `migration_status_for_path`. This module maps service short-names to their
//! XDG database paths and dispatches to the right runner.

use anyhow::{Context, Result};
use colored::Colorize;

/// Services that have SeaORM-managed SQLite databases.
pub const DB_SERVICES: &[DbService] = &[
    DbService { name: "memory", project: "agentd-memory", db_file: "memory.db" },
    DbService { name: "notify", project: "agentd-notify", db_file: "notify.db" },
    DbService { name: "orchestrator", project: "agentd-orchestrator", db_file: "orchestrator.db" },
    DbService { name: "communicate", project: "agentd-communicate", db_file: "communicate.db" },
];

pub struct DbService {
    /// Short name used in `--service` flag (e.g., `"notify"`)
    pub name: &'static str,
    /// XDG project name for database path resolution (e.g., `"agentd-notify"`)
    pub project: &'static str,
    /// Database filename (e.g., `"notify.db"`)
    pub db_file: &'static str,
}

/// Resolve the target services from an optional `--service` filter.
///
/// Returns all [`DB_SERVICES`] when `service` is `None`, or the single matching
/// entry when a name is provided.
pub fn resolve_services(service: Option<&str>) -> Result<Vec<&'static DbService>> {
    match service {
        Some(name) => {
            let svc = DB_SERVICES.iter().find(|s| s.name == name).with_context(|| {
                format!(
                    "Unknown service '{name}'. Valid: {}",
                    DB_SERVICES.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
                )
            })?;
            Ok(vec![svc])
        }
        None => Ok(DB_SERVICES.iter().collect()),
    }
}

/// Apply all pending SeaORM migrations for the specified service (or all
/// services if `service` is `None`). Creates the database file if needed.
pub async fn migrate(service: Option<&str>) -> Result<()> {
    let services = resolve_services(service)?;

    println!("{}", "Applying migrations...".blue().bold());
    println!();

    for svc in services {
        let db_path = agentd_common::storage::get_db_path(svc.project, svc.db_file)?;
        print!("  {} {} … ", "→".cyan(), svc.name.green());

        let result = match svc.name {
            "memory" => memory_api::apply_migrations_for_path(&db_path).await,
            "notify" => notify::apply_migrations_for_path(&db_path).await,
            "orchestrator" => orchestrator::apply_migrations_for_path(&db_path).await,
            "communicate" => communicate::apply_migrations_for_path(&db_path).await,
            _ => anyhow::bail!("No migration runner registered for service '{}'", svc.name),
        };

        match result {
            Ok(()) => println!("{}", "✓ up to date".green()),
            Err(e) => {
                println!("{}", "✗ failed".red());
                eprintln!("    {}", e);
            }
        }
    }

    println!();
    println!("{}", "Migration complete.".green().bold());
    Ok(())
}

/// Print the current migration status (applied / pending) for the specified
/// service (or all services).
pub async fn migrate_status(service: Option<&str>) -> Result<()> {
    let services = resolve_services(service)?;

    println!("{}", "Migration Status:".blue().bold());
    println!();

    for svc in services {
        println!("  {} {}:", "◆".cyan(), svc.name.green().bold());

        let db_path = agentd_common::storage::get_db_path(svc.project, svc.db_file)?;

        if !db_path.exists() {
            println!("    {} database not found — no migrations applied", "⚠".yellow());
            println!("    path: {}", db_path.display().to_string().bright_black());
            continue;
        }

        let result = match svc.name {
            "memory" => memory_api::migration_status_for_path(&db_path).await,
            "notify" => notify::migration_status_for_path(&db_path).await,
            "orchestrator" => orchestrator::migration_status_for_path(&db_path).await,
            "communicate" => communicate::migration_status_for_path(&db_path).await,
            _ => anyhow::bail!("No migration runner registered for service '{}'", svc.name),
        };

        match result {
            Ok(statuses) => {
                for (name, applied) in &statuses {
                    let (icon, label) = if *applied {
                        ("✓".green(), "applied".green())
                    } else {
                        ("○".yellow(), "pending".yellow())
                    };
                    println!("    {} {} {}", icon, label, name.bright_black());
                }
                let applied_count = statuses.iter().filter(|(_, a)| *a).count();
                let pending_count = statuses.len() - applied_count;
                println!(
                    "    {} applied, {} pending",
                    applied_count.to_string().green(),
                    if pending_count > 0 {
                        pending_count.to_string().yellow()
                    } else {
                        pending_count.to_string().green()
                    }
                );
            }
            Err(e) => {
                eprintln!("    {} failed to read status: {}", "✗".red(), e);
            }
        }
        println!();
    }

    Ok(())
}
