//! Migration: add the `mcp_servers` column to the `agents` table.
//!
//! Nullable TEXT holding a JSON-serialized `HashMap<String, McpServerConfig>`
//! (server name → { command, args, env }). `NULL` means the agent has no MCP
//! servers configured and claude is launched without `--mcp-config`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        if let Err(e) = db
            .execute_unprepared("ALTER TABLE agents ADD COLUMN mcp_servers TEXT DEFAULT NULL")
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
        // the column in place on rollback for simplicity.
        Ok(())
    }
}
