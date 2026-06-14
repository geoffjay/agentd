//! Migration: add `organization_id` column to the `memory_entries` table.
//!
//! NULL means "no org scope" (dev/test mode without gateway).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let stmt = "ALTER TABLE memory_entries ADD COLUMN organization_id TEXT";
        if let Err(e) = db.execute_unprepared(stmt).await {
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
