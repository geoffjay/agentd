//! Migration: drop the Docker-backend config columns from the `agents` table.
//!
//! The Docker execution backend was removed from agentd, so the per-agent
//! Docker settings it introduced are no longer read or written. This migration
//! drops the now-unused columns:
//! - `network_policy` (added by `m20250311_000004_add_network_policy`)
//! - `docker_image`, `extra_mounts`, `resource_limits`
//!   (added by `m20250312_000005_add_docker_config`)
//!
//! DROP COLUMN requires SQLite >= 3.35.0 (2021). Each drop is idempotent:
//! a missing column is ignored so the migration is safe to re-run and safe on
//! databases where the columns were never present.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        for stmt in [
            "ALTER TABLE agents DROP COLUMN network_policy",
            "ALTER TABLE agents DROP COLUMN docker_image",
            "ALTER TABLE agents DROP COLUMN extra_mounts",
            "ALTER TABLE agents DROP COLUMN resource_limits",
        ] {
            if let Err(e) = db.execute_unprepared(stmt).await {
                // Idempotent: ignore if the column is already gone.
                if !e.to_string().contains("no such column") {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // The Docker backend was removed; there is nothing meaningful to
        // restore. Leave the columns dropped on rollback.
        Ok(())
    }
}
