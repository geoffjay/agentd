//! Migration: add `task_json` column to the `dispatch_log` table.
//!
//! Adds one column:
//! - `task_json` (TEXT NULLABLE): JSON-serialized `Task` whose variables were
//!   rendered into `prompt_sent`. Enables re-triggering a dispatch from the UI
//!   with the original input values prefilled. NULL for records created before
//!   this migration. Note this duplicates some source-derived content already
//!   present in `prompt_sent`; growth impact is marginal.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = "ALTER TABLE dispatch_log ADD COLUMN task_json TEXT";
        if let Err(e) = db.execute_unprepared(stmt).await {
            // Idempotent: ignore if column already exists (e.g., re-run).
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite < 3.35.0 does not support DROP COLUMN.
        Ok(())
    }
}
