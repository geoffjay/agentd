//! Migration: rename `source_type` / `source_config` columns in the
//! `workflows` table to `trigger_type` / `trigger_config`.
//!
//! SQLite does not support `ALTER TABLE … RENAME COLUMN` before 3.25.0,
//! so we use the copy-to-new-table approach for maximum compatibility:
//!
//! 1. Create `workflows_new` with the updated column names.
//! 2. Copy all rows, mapping old columns to new names.
//! 3. Drop the old `workflows` table.
//! 4. Rename `workflows_new` → `workflows`.
//! 5. Re-create the unique index on `name`.
//!
//! The `down()` migration reverses the rename.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// SQL to copy the workflows table with renamed columns (old → new).
const UP_SQL: &[&str] = &[
    // 1. Create the replacement table with new column names.
    "CREATE TABLE IF NOT EXISTS workflows_new (
        id TEXT NOT NULL PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        agent_id TEXT NOT NULL,
        trigger_type TEXT NOT NULL,
        trigger_config TEXT NOT NULL,
        prompt_template TEXT NOT NULL,
        poll_interval_secs INTEGER NOT NULL DEFAULT 60,
        enabled INTEGER NOT NULL DEFAULT 1,
        tool_policy TEXT NOT NULL DEFAULT '{\"mode\":\"allow_all\"}',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    // 2. Copy data from old table → new table.
    "INSERT INTO workflows_new (
        id, name, agent_id, trigger_type, trigger_config,
        prompt_template, poll_interval_secs, enabled, tool_policy,
        created_at, updated_at
    )
    SELECT
        id, name, agent_id, source_type, source_config,
        prompt_template, poll_interval_secs, enabled, tool_policy,
        created_at, updated_at
    FROM workflows",
    // 3. Drop the old table.
    "DROP TABLE workflows",
    // 4. Rename the new table.
    "ALTER TABLE workflows_new RENAME TO workflows",
];

/// SQL to reverse the rename (new → old).
const DOWN_SQL: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS workflows_old (
        id TEXT NOT NULL PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        agent_id TEXT NOT NULL,
        source_type TEXT NOT NULL,
        source_config TEXT NOT NULL,
        prompt_template TEXT NOT NULL,
        poll_interval_secs INTEGER NOT NULL DEFAULT 60,
        enabled INTEGER NOT NULL DEFAULT 1,
        tool_policy TEXT NOT NULL DEFAULT '{\"mode\":\"allow_all\"}',
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )",
    "INSERT INTO workflows_old (
        id, name, agent_id, source_type, source_config,
        prompt_template, poll_interval_secs, enabled, tool_policy,
        created_at, updated_at
    )
    SELECT
        id, name, agent_id, trigger_type, trigger_config,
        prompt_template, poll_interval_secs, enabled, tool_policy,
        created_at, updated_at
    FROM workflows",
    "DROP TABLE workflows",
    "ALTER TABLE workflows_old RENAME TO workflows",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for stmt in UP_SQL {
            db.execute_unprepared(stmt).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for stmt in DOWN_SQL {
            db.execute_unprepared(stmt).await?;
        }
        Ok(())
    }
}
