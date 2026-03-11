//! Migration: add `network_policy` column to the `agents` table.
//!
//! Stores the optional network policy as a nullable TEXT column. Existing
//! rows get `NULL`, which the application layer interprets as "use the
//! backend default" (i.e., `NetworkPolicy::Internet`).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Add network_policy as a nullable TEXT column. NULL means "use default".
        if let Err(e) = db
            .execute_unprepared("ALTER TABLE agents ADD COLUMN network_policy TEXT DEFAULT NULL")
            .await
        {
            // Idempotent: ignore if column already exists (e.g., re-run).
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite < 3.35.0 does not support DROP COLUMN, so we leave
        // network_policy in place on rollback for simplicity.
        Ok(())
    }
}
