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
//!
//! # Usage
//!
//! ```bash
//! # Backfill all service databases with a specific org ID
//! agent admin backfill-tenant --org-id acme-corp
//!
//! # Dry run to preview affected row counts without modifying data
//! agent admin backfill-tenant --org-id acme-corp --dry-run
//! ```

use agentd_common::storage::{create_connection, get_db_path};
use agentd_install::migrate::DB_SERVICES;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use sea_orm::{ConnectionTrait, Statement, Value};

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
}

impl AdminCommand {
    /// Execute the admin subcommand.
    pub async fn execute(&self, json: bool) -> Result<()> {
        match self {
            AdminCommand::BackfillTenant { org_id, dry_run } => {
                backfill_tenant(org_id, *dry_run, json).await
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
    ServiceTables { service: "orchestrator", tables: &["agents", "workflows", "projects"] },
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
}
