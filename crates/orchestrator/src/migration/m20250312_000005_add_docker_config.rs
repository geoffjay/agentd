//! Migration: add Docker-specific config columns to the `agents` table.
//!
//! Adds three nullable columns for per-agent Docker settings:
//! - `docker_image` (TEXT): custom image override
//! - `extra_mounts` (TEXT): JSON-serialized `Vec<VolumeMount>`
//! - `resource_limits` (TEXT): JSON-serialized `ResourceLimits`
//!
//! Existing rows get `NULL`, which the application layer interprets as
//! "use the backend defaults".

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        for stmt in [
            "ALTER TABLE agents ADD COLUMN docker_image TEXT DEFAULT NULL",
            "ALTER TABLE agents ADD COLUMN extra_mounts TEXT DEFAULT NULL",
            "ALTER TABLE agents ADD COLUMN resource_limits TEXT DEFAULT NULL",
        ] {
            if let Err(e) = db.execute_unprepared(stmt).await {
                // Idempotent: ignore if column already exists (e.g., re-run).
                if !e.to_string().contains("duplicate column name") {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite < 3.35.0 does not support DROP COLUMN, so we leave
        // the columns in place on rollback for simplicity.
        Ok(())
    }
}
