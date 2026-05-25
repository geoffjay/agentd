//! Migration: add monotonic `seq` column to `conversation_events`.
//!
//! The orchestrator's snapshot+live streaming protocol relies on a strictly
//! monotonic per-agent sequence number to dedupe events across the
//! history/live boundary. Existing rows are backfilled with
//! `ROW_NUMBER() OVER (PARTITION BY agent_id ORDER BY created_at, id)`
//! so any prior data participates in the new ordering.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let add_column =
            "ALTER TABLE conversation_events ADD COLUMN seq INTEGER NOT NULL DEFAULT 0";
        if let Err(e) = db.execute_unprepared(add_column).await {
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        let backfill = "
            WITH ranked AS (
                SELECT id, ROW_NUMBER() OVER (
                    PARTITION BY agent_id
                    ORDER BY created_at, id
                ) AS rn
                FROM conversation_events
            )
            UPDATE conversation_events
            SET seq = (SELECT rn FROM ranked WHERE ranked.id = conversation_events.id)
            WHERE seq = 0
        ";
        db.execute_unprepared(backfill).await?;

        let create_index = "
            CREATE UNIQUE INDEX IF NOT EXISTS idx_conv_events_agent_seq
            ON conversation_events (agent_id, seq)
        ";
        db.execute_unprepared(create_index).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP INDEX IF EXISTS idx_conv_events_agent_seq").await?;
        // SQLite < 3.35.0 lacks DROP COLUMN; leave the column in place.
        Ok(())
    }
}
