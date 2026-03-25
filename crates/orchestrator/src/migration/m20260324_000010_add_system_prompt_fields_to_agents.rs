//! Migration: add `system_prompt_file` and `append_system_prompt` columns to the `agents` table.
//!
//! Adds two columns:
//! - `system_prompt_file` (TEXT NULLABLE): path to a file whose contents replace or append to
//!   the default system prompt. Mutually exclusive with the existing `system_prompt` column.
//! - `append_system_prompt` (INTEGER NOT NULL DEFAULT 0): boolean flag. When non-zero, the
//!   `--append-system-prompt` / `--append-system-prompt-file` flag is used instead of
//!   `--system-prompt` / `--system-prompt-file`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        for stmt in [
            "ALTER TABLE agents ADD COLUMN system_prompt_file TEXT",
            "ALTER TABLE agents ADD COLUMN append_system_prompt INTEGER NOT NULL DEFAULT 0",
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
        tracing::warn!(
            "Migration {} rolled back but columns system_prompt_file and \
             append_system_prompt remain in the agents table \
             (SQLite does not support DROP COLUMN before 3.35.0)",
            self.name()
        );
        Ok(())
    }
}
