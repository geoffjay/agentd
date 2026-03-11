//! Migration: rename `tmux_session` to `session_id` and add `backend_type`.
//!
//! SQLite does not support `RENAME COLUMN` prior to version 3.25.0, and even
//! then the SeaORM migration API doesn't expose it directly.  We use raw SQL
//! for the column rename and add `backend_type` via ALTER TABLE.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // -----------------------------------------------------------------
        // Rename tmux_session → session_id
        //
        // SQLite 3.25+ supports ALTER TABLE ... RENAME COLUMN.  For older
        // versions this would need a full table rebuild, but all supported
        // platforms ship 3.25+.
        // -----------------------------------------------------------------
        db.execute_unprepared("ALTER TABLE agents RENAME COLUMN tmux_session TO session_id")
            .await?;

        // -----------------------------------------------------------------
        // Add backend_type column (nullable, default 'tmux')
        //
        // Existing rows get 'tmux' automatically since that was the only
        // backend before this migration.
        // -----------------------------------------------------------------
        if let Err(e) = db
            .execute_unprepared("ALTER TABLE agents ADD COLUMN backend_type TEXT DEFAULT 'tmux'")
            .await
        {
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Reverse the rename.
        db.execute_unprepared("ALTER TABLE agents RENAME COLUMN session_id TO tmux_session")
            .await?;

        // SQLite < 3.35.0 does not support DROP COLUMN, so we leave
        // backend_type in place on rollback for simplicity.

        Ok(())
    }
}
