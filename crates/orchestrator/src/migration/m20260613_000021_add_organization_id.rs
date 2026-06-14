//! Migration: add `organization_id` column to tenant-scoped tables.
//!
//! Adds `organization_id TEXT` (nullable) to:
//! - `agents`
//! - `workflows`
//! - `dispatch_log`
//! - `projects`
//!
//! NULL means "no org scope" (dev/test mode without gateway).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmts = [
            "ALTER TABLE agents ADD COLUMN organization_id TEXT",
            "ALTER TABLE workflows ADD COLUMN organization_id TEXT",
            "ALTER TABLE dispatch_log ADD COLUMN organization_id TEXT",
            "ALTER TABLE projects ADD COLUMN organization_id TEXT",
        ];

        for stmt in &stmts {
            if let Err(e) = db.execute_unprepared(stmt).await {
                if !e.to_string().contains("duplicate column name") {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite < 3.35.0 does not support DROP COLUMN.
        Ok(())
    }
}
