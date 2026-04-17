//! Migration: add `built_in` column to the `agents` table.
//!
//! Adds one column:
//! - `built_in` (INTEGER NOT NULL DEFAULT 0): distinguishes programmatically-created
//!   system agents (`1`) from user-created agents (`0`).  All existing rows receive
//!   the default value of `0` so that no data migration is needed.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = "ALTER TABLE agents ADD COLUMN built_in INTEGER NOT NULL DEFAULT 0";
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
