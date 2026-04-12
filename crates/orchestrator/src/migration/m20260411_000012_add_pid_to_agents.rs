//! Migration: add `pid` column to the `agents` table.
//!
//! Adds one column:
//! - `pid` (INTEGER NULLABLE): the OS process ID of the agent's subprocess.
//!   Used during startup reconciliation to check whether a surviving process
//!   from a previous orchestrator run is still alive, avoiding duplicate spawns.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = "ALTER TABLE agents ADD COLUMN pid INTEGER";
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
