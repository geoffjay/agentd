//! Migration: add `rooms` column to the `agents` table.
//!
//! Adds one column:
//! - `rooms` (TEXT NOT NULL DEFAULT '[]'): JSON-serialized `Vec<String>`
//!   of communicate room names the agent should auto-join on connect.
//!
//! Existing rows default to an empty JSON array (`[]`), preserving
//! backwards compatibility (agents without rooms configured).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = "ALTER TABLE agents ADD COLUMN rooms TEXT NOT NULL DEFAULT '[]'";
        if let Err(e) = db.execute_unprepared(stmt).await {
            // Idempotent: ignore if column already exists (e.g., re-run).
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite < 3.35.0 does not support DROP COLUMN, so we leave
        // the column in place on rollback for simplicity.
        Ok(())
    }
}
