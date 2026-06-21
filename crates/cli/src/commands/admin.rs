//! Admin commands for agentd management operations.
//!
//! Provides privileged maintenance commands that operate directly on service
//! databases. These commands are intended for operators and should be run
//! with appropriate care in production environments.
//!
//! # Available Commands
//!
//! - **backfill-tenant** — Assign an `organization_id` to all existing rows
//!   that have a NULL value, backfilling legacy unscoped data.
//! - **backfill-projects** — Copy project rows from orchestrator's database
//!   into core's database, preserving UUIDs.
//!
//! # Usage
//!
//! ```bash
//! # Backfill all service databases with a specific org ID
//! agent admin backfill-tenant --org-id acme-corp
//!
//! # Dry run to preview affected row counts without modifying data
//! agent admin backfill-tenant --org-id acme-corp --dry-run
//!
//! # Copy all orchestrator projects into core
//! agent admin backfill-projects
//!
//! # Dry-run scoped to one organization
//! agent admin backfill-projects --org-id acme-corp --dry-run
//! ```

use std::collections::HashSet;

use agentd_common::storage::{create_connection, get_db_path};
use agentd_install::migrate::DB_SERVICES;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use sea_orm::{ConnectionTrait, FromQueryResult, Statement, Value};

/// Admin subcommands.
#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    /// Backfill NULL organization_id values across all service databases.
    ///
    /// After the tenant scoping migration adds the `organization_id` column,
    /// existing rows will have NULL values. This command assigns a specific
    /// organization ID to all such rows so they are accessible to the
    /// tenant-aware queries.
    ///
    /// **When to use:**
    /// - After upgrading to a version that adds multi-tenancy support
    /// - When migrating a single-tenant install to multi-tenant
    /// - When assigning legacy data to a specific organization
    ///
    /// **NULL handling policy:** During the transition period, list queries
    /// that include a tenant ID will return rows where `organization_id`
    /// matches the tenant OR is NULL (legacy data). After running this
    /// command, all rows are fully scoped and the NULL fallback is no longer
    /// needed.
    #[command(name = "backfill-tenant")]
    BackfillTenant {
        /// The organization ID to assign to all unscoped (NULL) rows.
        #[arg(long)]
        org_id: String,

        /// Preview the number of affected rows without modifying any data.
        #[arg(long)]
        dry_run: bool,
    },

    /// Copy project rows from orchestrator's database into core's database.
    ///
    /// Phase 2 of the projects-to-core migration: after core's `projects`
    /// table exists (Phase 1), this command seeds it with all existing rows
    /// from orchestrator, preserving UUIDs so that foreign-key references in
    /// other services continue to resolve.
    ///
    /// The command is **idempotent**: rows already present in core (matched by
    /// `id`) are silently skipped, so it is safe to run more than once.
    ///
    /// **When to use:**
    /// - After upgrading to a release that adds `projects` to core
    /// - Before repointing consumers (communicate, knowledge, orchestrator)
    ///   from the orchestrator project API to the core project API
    #[command(name = "backfill-projects")]
    BackfillProjects {
        /// Preview which rows would be inserted without writing anything.
        #[arg(long)]
        dry_run: bool,

        /// Scope the backfill to a specific organization.
        ///
        /// When provided, only projects with a matching `organization_id`
        /// **or** a NULL `organization_id` (legacy data) are copied.
        /// When omitted, all orchestrator projects are copied.
        #[arg(long)]
        org_id: Option<String>,
    },
}

impl AdminCommand {
    /// Execute the admin subcommand.
    pub async fn execute(&self, json: bool) -> Result<()> {
        match self {
            AdminCommand::BackfillTenant { org_id, dry_run } => {
                backfill_tenant(org_id, *dry_run, json).await
            }
            AdminCommand::BackfillProjects { dry_run, org_id } => {
                backfill_projects(org_id.as_deref(), *dry_run, json).await
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Table definitions per service
// ---------------------------------------------------------------------------

/// Tables and their `organization_id` columns per service database.
struct ServiceTables {
    /// Database service name (matches `DB_SERVICES`)
    service: &'static str,
    /// Tables in this service that have an `organization_id` column
    tables: &'static [&'static str],
}

const SCOPED_TABLES: &[ServiceTables] = &[
    ServiceTables { service: "orchestrator", tables: &["agents", "workflows"] },
    ServiceTables { service: "core", tables: &["projects"] },
    ServiceTables { service: "notify", tables: &["notifications"] },
    ServiceTables { service: "communicate", tables: &["rooms"] },
    ServiceTables { service: "memory", tables: &["memory_entries"] },
];

// ---------------------------------------------------------------------------
// backfill_tenant
// ---------------------------------------------------------------------------

/// Count rows with NULL organization_id in a table.
///
/// `table` must come from the hardcoded `SCOPED_TABLES` constant — it is
/// never user-supplied and is safe to interpolate into the statement string.
async fn count_null_org_rows(
    db: &sea_orm::DatabaseConnection,
    table: &str,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::FromQueryResult;

    #[derive(Debug, sea_orm::FromQueryResult)]
    struct CountRow {
        cnt: i64,
    }

    let result = CountRow::find_by_statement(Statement::from_string(
        db.get_database_backend(),
        format!("SELECT COUNT(*) AS cnt FROM {} WHERE organization_id IS NULL", table),
    ))
    .one(db)
    .await?;

    Ok(result.map(|r| r.cnt as u64).unwrap_or(0))
}

/// Assign `org_id` to all NULL `organization_id` rows in a table.
///
/// Uses a parameterized statement so that user-supplied `org_id` values
/// (e.g. those containing single quotes) cannot inject SQL.
///
/// `table` must come from the hardcoded `SCOPED_TABLES` constant — it is
/// never user-supplied and is safe to interpolate into the statement string.
async fn update_null_org_rows(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    org_id: &str,
) -> Result<u64, sea_orm::DbErr> {
    let stmt = Statement::from_sql_and_values(
        db.get_database_backend(),
        format!("UPDATE {} SET organization_id = ? WHERE organization_id IS NULL", table),
        vec![Value::from(org_id.to_owned())],
    );
    let result = db.execute(stmt).await?;
    Ok(result.rows_affected())
}

#[derive(serde::Serialize)]
struct BackfillResult {
    service: String,
    table: String,
    rows_affected: u64,
    dry_run: bool,
}

async fn backfill_tenant(org_id: &str, dry_run: bool, json: bool) -> Result<()> {
    if org_id.is_empty() {
        anyhow::bail!("--org-id must not be empty");
    }

    let mut results: Vec<BackfillResult> = Vec::new();

    for scoped in SCOPED_TABLES {
        // Look up the DB service definition to get path information.
        let svc_def = DB_SERVICES.iter().find(|s| s.name == scoped.service);
        let Some(svc) = svc_def else {
            eprintln!("warning: service '{}' not found in DB_SERVICES — skipping", scoped.service);
            continue;
        };

        let db_path = get_db_path(svc.project, svc.db_file)
            .with_context(|| format!("cannot resolve DB path for {}", scoped.service))?;

        if !db_path.exists() {
            if !json {
                println!(
                    "  {} {} (database not found — skipping)",
                    "⚠".yellow(),
                    scoped.service.bright_black()
                );
            }
            continue;
        }

        let db = create_connection(&db_path)
            .await
            .with_context(|| format!("failed to open {} database", scoped.service))?;

        for &table in scoped.tables {
            let rows = if dry_run {
                count_null_org_rows(&db, table)
                    .await
                    .with_context(|| format!("count failed on {}.{}", scoped.service, table))?
            } else {
                update_null_org_rows(&db, table, org_id)
                    .await
                    .with_context(|| format!("update failed on {}.{}", scoped.service, table))?
            };

            results.push(BackfillResult {
                service: scoped.service.to_string(),
                table: table.to_string(),
                rows_affected: rows,
                dry_run,
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    let mode_label = if dry_run { " (dry-run)" } else { "" };
    println!("{}", format!("Backfill tenant: org_id = {org_id}{mode_label}").blue().bold());
    println!("{}", "=".repeat(60).cyan());

    let mut total_rows = 0u64;
    for r in &results {
        let verb = if dry_run { "would update" } else { "updated" };
        let icon = if r.rows_affected > 0 { "✅" } else { "  " };
        println!(
            "  {} {:<20} {:<25} {} {} rows",
            icon,
            r.service.bold(),
            r.table.bright_black(),
            verb,
            r.rows_affected.to_string().green().bold()
        );
        total_rows += r.rows_affected;
    }

    println!();
    let summary_verb = if dry_run { "Would update" } else { "Updated" };
    println!(
        "{} {} rows across {} tables",
        summary_verb,
        total_rows.to_string().green().bold(),
        results.len()
    );

    if dry_run {
        println!("{}", "Run without --dry-run to apply the backfill.".yellow());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// backfill_projects
// ---------------------------------------------------------------------------

/// A project row read from orchestrator's `projects` table.
#[derive(Debug, FromQueryResult)]
struct OrchestratorProjectRow {
    id: String,
    name: String,
    description: Option<String>,
    organization_id: Option<String>,
    created_at: String,
    updated_at: String,
}

/// Per-project outcome reported by [`run_backfill_projects`].
#[derive(serde::Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ProjectBackfillStatus {
    /// Row was inserted into core's `projects` table.
    Inserted,
    /// Row already existed in core (matched by `id`); skipped.
    Skipped,
    /// Dry-run mode: row would be inserted.
    WouldInsert,
    /// Insert failed (e.g. unique-name conflict with a different id).
    Error,
}

/// Per-project result returned by [`run_backfill_projects`].
#[derive(serde::Serialize, Clone, Debug)]
pub struct ProjectBackfillResult {
    pub id: String,
    pub name: String,
    pub status: ProjectBackfillStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Core logic for the backfill-projects command.
///
/// Separated from [`backfill_projects`] so it can be exercised in tests with
/// in-memory SQLite connections instead of real service databases.
pub async fn run_backfill_projects(
    orch_db: &sea_orm::DatabaseConnection,
    core_db: &sea_orm::DatabaseConnection,
    org_id: Option<&str>,
    dry_run: bool,
) -> Result<Vec<ProjectBackfillResult>> {
    // Read projects from orchestrator (with optional org scope).
    let rows = if let Some(oid) = org_id {
        OrchestratorProjectRow::find_by_statement(Statement::from_sql_and_values(
            orch_db.get_database_backend(),
            "SELECT id, name, description, organization_id, created_at, updated_at \
             FROM projects \
             WHERE organization_id = ? OR organization_id IS NULL",
            vec![Value::from(oid.to_owned())],
        ))
        .all(orch_db)
        .await
        .context("failed to read orchestrator projects")?
    } else {
        OrchestratorProjectRow::find_by_statement(Statement::from_string(
            orch_db.get_database_backend(),
            "SELECT id, name, description, organization_id, created_at, updated_at \
             FROM projects",
        ))
        .all(orch_db)
        .await
        .context("failed to read orchestrator projects")?
    };

    // Load all project IDs already present in core so we can skip them cheaply.
    #[derive(FromQueryResult)]
    struct IdRow {
        id: String,
    }

    let existing: HashSet<String> = IdRow::find_by_statement(Statement::from_string(
        core_db.get_database_backend(),
        "SELECT id FROM projects",
    ))
    .all(core_db)
    .await
    .context("failed to read existing project IDs from core")?
    .into_iter()
    .map(|r| r.id)
    .collect();

    let mut results = Vec::with_capacity(rows.len());

    for row in rows {
        // Skip rows already present in core (idempotency).
        if existing.contains(&row.id) {
            results.push(ProjectBackfillResult {
                id: row.id,
                name: row.name,
                status: ProjectBackfillStatus::Skipped,
                error: None,
            });
            continue;
        }

        if dry_run {
            results.push(ProjectBackfillResult {
                id: row.id,
                name: row.name,
                status: ProjectBackfillStatus::WouldInsert,
                error: None,
            });
            continue;
        }

        let stmt = Statement::from_sql_and_values(
            core_db.get_database_backend(),
            "INSERT INTO projects \
             (id, name, description, organization_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
            vec![
                Value::from(row.id.clone()),
                Value::from(row.name.clone()),
                Value::from(row.description.clone()),
                Value::from(row.organization_id.clone()),
                Value::from(row.created_at.clone()),
                Value::from(row.updated_at.clone()),
            ],
        );

        match core_db.execute(stmt).await {
            Ok(_) => results.push(ProjectBackfillResult {
                id: row.id,
                name: row.name,
                status: ProjectBackfillStatus::Inserted,
                error: None,
            }),
            Err(e) => results.push(ProjectBackfillResult {
                id: row.id,
                name: row.name,
                status: ProjectBackfillStatus::Error,
                error: Some(e.to_string()),
            }),
        }
    }

    Ok(results)
}

/// Render backfill results to stdout in either JSON or human-readable form.
fn output_backfill_projects_results(
    results: &[ProjectBackfillResult],
    dry_run: bool,
    org_id: Option<&str>,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(results)?);
        return Ok(());
    }

    let mode_label = if dry_run { " (dry-run)" } else { "" };
    let scope_label = match org_id {
        Some(oid) => format!(": org_id = {oid}"),
        None => String::new(),
    };
    println!("{}", format!("Backfill projects{scope_label}{mode_label}").blue().bold());
    println!("{}", "=".repeat(60).cyan());

    let mut n_inserted = 0u64;
    let mut n_skipped = 0u64;
    let mut n_errors = 0u64;
    let mut n_would_insert = 0u64;

    for r in results {
        let (icon, verb) = match r.status {
            ProjectBackfillStatus::Inserted => {
                n_inserted += 1;
                ("✅", "inserted")
            }
            ProjectBackfillStatus::Skipped => {
                n_skipped += 1;
                ("  ", "skipped")
            }
            ProjectBackfillStatus::WouldInsert => {
                n_would_insert += 1;
                ("🔍", "would insert")
            }
            ProjectBackfillStatus::Error => {
                n_errors += 1;
                ("❌", "error")
            }
        };
        let colored_verb = match r.status {
            ProjectBackfillStatus::Error => verb.red().bold(),
            _ => verb.green().bold(),
        };
        println!("  {} {:<40} {}", icon, r.name.bright_black(), colored_verb);
        if let Some(err) = &r.error {
            println!("     error: {}", err.red());
        }
    }

    println!();
    let total = results.len();
    if dry_run {
        println!(
            "Total: {} rows found in orchestrator — {} would be inserted, {} already in core",
            total.to_string().green().bold(),
            n_would_insert.to_string().green().bold(),
            n_skipped.to_string().bright_black()
        );
        println!("{}", "Run without --dry-run to apply the backfill.".yellow());
    } else {
        println!(
            "Total: {} rows processed — {} inserted, {} skipped, {} errors",
            total.to_string().green().bold(),
            n_inserted.to_string().green().bold(),
            n_skipped.to_string().bright_black(),
            if n_errors > 0 { n_errors.to_string().red() } else { n_errors.to_string().green() }
        );
    }

    Ok(())
}

/// Open both service databases and run the project backfill.
async fn backfill_projects(org_id: Option<&str>, dry_run: bool, json: bool) -> Result<()> {
    // Reject an explicitly empty --org-id before touching any database.
    if let Some(id) = org_id {
        if id.is_empty() {
            anyhow::bail!("--org-id must not be empty");
        }
    }

    // Open orchestrator database.
    let orch_path = get_db_path("agentd-orchestrator", "orchestrator.db")
        .context("cannot resolve orchestrator database path")?;
    if !orch_path.exists() {
        anyhow::bail!(
            "orchestrator database not found at {}; is the orchestrator service initialized?",
            orch_path.display()
        );
    }
    let orch_db =
        create_connection(&orch_path).await.context("failed to open orchestrator database")?;

    // Open core database.
    let core_path =
        get_db_path("agentd-core", "core.db").context("cannot resolve core database path")?;
    if !core_path.exists() {
        anyhow::bail!(
            "core database not found at {}; is the core service initialized?",
            core_path.display()
        );
    }
    let core_db = create_connection(&core_path).await.context("failed to open core database")?;

    let results = run_backfill_projects(&orch_db, &core_db, org_id, dry_run).await?;
    output_backfill_projects_results(&results, dry_run, org_id, json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_common::storage::create_test_connection;
    use sea_orm::ConnectionTrait;

    /// Create a minimal table with an `organization_id` column in the given
    /// in-memory connection, then insert `null_count` rows with NULL
    /// `organization_id` and `scoped_count` rows with a known org ID.
    async fn seed_table(
        db: &sea_orm::DatabaseConnection,
        table: &str,
        null_count: usize,
        scoped_count: usize,
    ) {
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS {table} (id INTEGER PRIMARY KEY, organization_id TEXT)"
        ))
        .await
        .unwrap();

        for _ in 0..null_count {
            db.execute_unprepared(&format!("INSERT INTO {table} (organization_id) VALUES (NULL)"))
                .await
                .unwrap();
        }

        for _ in 0..scoped_count {
            db.execute_unprepared(&format!(
                "INSERT INTO {table} (organization_id) VALUES ('existing-org')"
            ))
            .await
            .unwrap();
        }
    }

    /// Count all rows in a table — used to verify no extra rows were inserted.
    async fn total_rows(db: &sea_orm::DatabaseConnection, table: &str) -> u64 {
        use sea_orm::FromQueryResult;

        #[derive(sea_orm::FromQueryResult)]
        struct C {
            n: i64,
        }

        let r = C::find_by_statement(Statement::from_string(
            db.get_database_backend(),
            format!("SELECT COUNT(*) AS n FROM {table}"),
        ))
        .one(db)
        .await
        .unwrap();

        r.map(|c| c.n as u64).unwrap_or(0)
    }

    /// Count rows where organization_id IS NULL.
    async fn null_rows(db: &sea_orm::DatabaseConnection, table: &str) -> u64 {
        count_null_org_rows(db, table).await.unwrap()
    }

    #[tokio::test]
    async fn count_returns_only_null_rows() {
        let (db, _tmp) = create_test_connection().await;
        seed_table(&db, "items", 3, 2).await;

        let count = null_rows(&db, "items").await;
        assert_eq!(count, 3, "should count only NULL organization_id rows");
    }

    #[tokio::test]
    async fn dry_run_does_not_mutate_data() {
        let (db, _tmp) = create_test_connection().await;
        seed_table(&db, "items", 4, 1).await;

        // Dry-run: count nulls without touching them
        let counted = count_null_org_rows(&db, "items").await.unwrap();
        assert_eq!(counted, 4);

        // Data must be unchanged after the count
        assert_eq!(null_rows(&db, "items").await, 4, "dry-run must not modify rows");
        assert_eq!(total_rows(&db, "items").await, 5, "total row count must be unchanged");
    }

    #[tokio::test]
    async fn update_assigns_org_id_to_null_rows_only() {
        let (db, _tmp) = create_test_connection().await;
        seed_table(&db, "items", 3, 2).await;

        let affected = update_null_org_rows(&db, "items", "acme-corp").await.unwrap();
        assert_eq!(affected, 3, "should update exactly the 3 NULL rows");

        // No more NULL rows
        assert_eq!(null_rows(&db, "items").await, 0, "no NULL rows should remain");

        // Existing-org rows must be untouched
        assert_eq!(total_rows(&db, "items").await, 5, "total row count must be unchanged");
    }

    #[tokio::test]
    async fn update_handles_org_id_with_special_characters() {
        let (db, _tmp) = create_test_connection().await;
        seed_table(&db, "items", 2, 0).await;

        // An org ID containing a single quote would break naive string interpolation.
        let affected = update_null_org_rows(&db, "items", "acme's-org").await.unwrap();
        assert_eq!(affected, 2);
        assert_eq!(null_rows(&db, "items").await, 0);
    }

    #[tokio::test]
    async fn update_zero_rows_when_table_already_scoped() {
        let (db, _tmp) = create_test_connection().await;
        seed_table(&db, "items", 0, 5).await;

        let affected = update_null_org_rows(&db, "items", "acme-corp").await.unwrap();
        assert_eq!(affected, 0, "nothing to update when all rows are already scoped");
    }

    #[tokio::test]
    async fn backfill_rejects_empty_org_id() {
        // The guard must fire before any database is touched, so this is safe to
        // run regardless of which service DBs exist in the environment.
        let err = backfill_tenant("", false, true).await.unwrap_err();
        assert!(
            err.to_string().contains("must not be empty"),
            "expected an empty-org-id error, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // backfill_projects tests
    // -----------------------------------------------------------------------

    /// Create the `projects` schema in an in-memory SQLite connection.
    async fn create_projects_table(db: &sea_orm::DatabaseConnection) {
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS projects (\
                id TEXT PRIMARY KEY, \
                name TEXT NOT NULL UNIQUE, \
                description TEXT, \
                organization_id TEXT, \
                created_at TEXT NOT NULL, \
                updated_at TEXT NOT NULL\
            )",
        )
        .await
        .unwrap();
    }

    /// Seed the `projects` table with rows of the form `(id, name, org_id)`.
    ///
    /// `org_id = None` → NULL in the database.
    ///
    /// Uses parameterized statements so that values containing special characters
    /// (e.g. single quotes) are handled safely, consistent with production paths.
    async fn seed_projects(db: &sea_orm::DatabaseConnection, rows: &[(&str, &str, Option<&str>)]) {
        create_projects_table(db).await;
        for (id, name, org_id) in rows {
            let stmt = Statement::from_sql_and_values(
                db.get_database_backend(),
                "INSERT INTO projects \
                 (id, name, description, organization_id, created_at, updated_at) \
                 VALUES (?, ?, NULL, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
                vec![
                    Value::from((*id).to_string()),
                    Value::from((*name).to_string()),
                    Value::from(org_id.map(|s| s.to_string())),
                ],
            );
            db.execute(stmt).await.unwrap();
        }
    }

    #[tokio::test]
    async fn backfill_projects_rejects_empty_org_id() {
        // Guard fires before any database path is resolved.
        let err = backfill_projects(Some(""), false, true).await.unwrap_err();
        assert!(
            err.to_string().contains("must not be empty"),
            "expected an empty-org-id error, got: {err}"
        );
    }

    #[tokio::test]
    async fn backfill_projects_inserts_all_rows_first_run() {
        let (orch_db, _orch_tmp) = create_test_connection().await;
        let (core_db, _core_tmp) = create_test_connection().await;

        seed_projects(&orch_db, &[("uuid-1", "Alpha", Some("org-a")), ("uuid-2", "Beta", None)])
            .await;
        create_projects_table(&core_db).await;

        let results = run_backfill_projects(&orch_db, &core_db, None, false).await.unwrap();

        assert_eq!(results.len(), 2, "should process both orchestrator rows");
        assert!(
            results.iter().all(|r| r.status == ProjectBackfillStatus::Inserted),
            "both rows should be inserted on first run"
        );
    }

    #[tokio::test]
    async fn backfill_projects_idempotent() {
        let (orch_db, _orch_tmp) = create_test_connection().await;
        let (core_db, _core_tmp) = create_test_connection().await;

        seed_projects(
            &orch_db,
            &[("uuid-1", "Alpha", Some("org-a")), ("uuid-2", "Beta", Some("org-b"))],
        )
        .await;
        create_projects_table(&core_db).await;

        // First run: both rows should be inserted.
        let first = run_backfill_projects(&orch_db, &core_db, None, false).await.unwrap();
        assert!(
            first.iter().all(|r| r.status == ProjectBackfillStatus::Inserted),
            "first run should insert all rows"
        );

        // Second run: both rows already exist → all skipped.
        let second = run_backfill_projects(&orch_db, &core_db, None, false).await.unwrap();
        assert_eq!(second.len(), 2, "second run should still process both rows");
        assert!(
            second.iter().all(|r| r.status == ProjectBackfillStatus::Skipped),
            "second run should skip all rows (already in core)"
        );
    }

    #[tokio::test]
    async fn backfill_projects_dry_run_does_not_insert() {
        let (orch_db, _orch_tmp) = create_test_connection().await;
        let (core_db, _core_tmp) = create_test_connection().await;

        seed_projects(&orch_db, &[("uuid-1", "Alpha", None)]).await;
        create_projects_table(&core_db).await;

        let results =
            run_backfill_projects(&orch_db, &core_db, None, /* dry_run */ true).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ProjectBackfillStatus::WouldInsert);

        // Core must remain empty.
        assert_eq!(total_rows(&core_db, "projects").await, 0, "dry-run must not insert anything");
    }

    #[tokio::test]
    async fn backfill_projects_org_filter_includes_null_rows() {
        let (orch_db, _orch_tmp) = create_test_connection().await;
        let (core_db, _core_tmp) = create_test_connection().await;

        seed_projects(
            &orch_db,
            &[
                ("uuid-1", "OrgA Project", Some("org-a")),
                ("uuid-2", "OrgB Project", Some("org-b")),
                ("uuid-3", "Legacy Project", None),
            ],
        )
        .await;
        create_projects_table(&core_db).await;

        // Filter to org-a: should include OrgA Project AND Legacy Project (NULL).
        let results =
            run_backfill_projects(&orch_db, &core_db, Some("org-a"), false).await.unwrap();

        assert_eq!(results.len(), 2, "org filter should return org-a + NULL rows");
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"OrgA Project"));
        assert!(names.contains(&"Legacy Project"));
        assert!(!names.contains(&"OrgB Project"), "org-b project must not appear");
    }

    #[tokio::test]
    async fn backfill_projects_name_collision_reports_error() {
        let (orch_db, _orch_tmp) = create_test_connection().await;
        let (core_db, _core_tmp) = create_test_connection().await;

        // Orchestrator has "SharedName" with id "uuid-orch".
        seed_projects(&orch_db, &[("uuid-orch", "SharedName", None)]).await;

        // Core already has "SharedName" but under a *different* id.
        // Because the id doesn't match, the skip-by-id guard won't fire,
        // and the INSERT will hit the UNIQUE constraint on `name`.
        seed_projects(&core_db, &[("uuid-core", "SharedName", None)]).await;

        let results = run_backfill_projects(&orch_db, &core_db, None, false).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].status,
            ProjectBackfillStatus::Error,
            "name collision should yield Error status, not a panic"
        );
        assert!(results[0].error.is_some(), "error field must contain the database error message");
    }
}
