//! Migration: add `auto_clear_threshold` to `agents` and create `agent_usage_sessions`.
//!
//! - Adds a nullable `auto_clear_threshold INTEGER` column to the existing
//!   `agents` table using a SQLite-safe idempotent ALTER TABLE approach.
//! - Creates the `agent_usage_sessions` table for per-session token/cost tracking.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // -----------------------------------------------------------------
        // Add auto_clear_threshold to agents (idempotent)
        //
        // SQLite prior to 3.37.0 does not support "ADD COLUMN IF NOT EXISTS",
        // so we attempt the ALTER TABLE and silently swallow the
        // "duplicate column name" error in case the column already exists.
        // -----------------------------------------------------------------
        let db = manager.get_connection();
        if let Err(e) = db
            .execute_unprepared("ALTER TABLE agents ADD COLUMN auto_clear_threshold INTEGER")
            .await
        {
            if !e.to_string().contains("duplicate column name") {
                return Err(e);
            }
        }

        // -----------------------------------------------------------------
        // agent_usage_sessions table
        // TODO: add SeaORM entity in follow-up
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(AgentUsageSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AgentUsageSessions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AgentUsageSessions::AgentId).string().not_null())
                    .col(
                        ColumnDef::new(AgentUsageSessions::SessionNumber)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::InputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::OutputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::CacheReadInputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::CacheCreationInputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::TotalCostUsd)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::NumTurns)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::DurationMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::DurationApiMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AgentUsageSessions::ResultCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(AgentUsageSessions::StartedAt).string().not_null())
                    .col(ColumnDef::new(AgentUsageSessions::EndedAt).string().null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(AgentUsageSessions::Table, AgentUsageSessions::AgentId)
                            .to(Agents::Table, Agents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on agent_id for fast per-agent lookups.
        manager
            .create_index(
                Index::create()
                    .name("idx_usage_sessions_agent_id")
                    .table(AgentUsageSessions::Table)
                    .col(AgentUsageSessions::AgentId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(AgentUsageSessions::Table).to_owned()).await?;
        // To keep the rollback simple the auto_clear_threshold column is left
        // in place (SQLite < 3.35.0 does not support DROP COLUMN).
        Ok(())
    }
}

/// Iden enum for the `agents` table (referenced for the foreign key).
#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
}

/// Iden enum for the `agent_usage_sessions` table.
#[derive(DeriveIden)]
enum AgentUsageSessions {
    Table,
    Id,
    AgentId,
    SessionNumber,
    InputTokens,
    OutputTokens,
    CacheReadInputTokens,
    CacheCreationInputTokens,
    TotalCostUsd,
    NumTurns,
    DurationMs,
    DurationApiMs,
    ResultCount,
    StartedAt,
    EndedAt,
}
