//! Migration: replace old check-based questions table with agent-driven Q&A schema.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the old table entirely — the old schema is incompatible.
        manager
            .drop_table(Table::drop().table(Questions::Table).if_exists().to_owned())
            .await?;

        // Create new agent-driven questions table.
        manager
            .create_table(
                Table::create()
                    .table(Questions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Questions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Questions::AgentId).string().not_null())
                    .col(ColumnDef::new(Questions::WorkflowId).string().null())
                    .col(ColumnDef::new(Questions::DispatchId).string().null())
                    .col(ColumnDef::new(Questions::Category).string().null())
                    .col(ColumnDef::new(Questions::Question).string().not_null())
                    .col(ColumnDef::new(Questions::Context).string().null())
                    .col(
                        ColumnDef::new(Questions::Priority)
                            .string()
                            .not_null()
                            .default("normal"),
                    )
                    .col(
                        ColumnDef::new(Questions::Status)
                            .string()
                            .not_null()
                            .default("Pending"),
                    )
                    .col(ColumnDef::new(Questions::Answer).string().null())
                    .col(ColumnDef::new(Questions::AskedAt).string().not_null())
                    .col(ColumnDef::new(Questions::AnsweredAt).string().null())
                    .col(ColumnDef::new(Questions::ExpiresAt).string().null())
                    .to_owned(),
            )
            .await?;

        // Index on status for filtering pending/answered questions.
        manager
            .create_index(
                Index::create()
                    .name("idx_questions_status")
                    .table(Questions::Table)
                    .col(Questions::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on agent_id for filtering by asking agent.
        manager
            .create_index(
                Index::create()
                    .name("idx_questions_agent_id")
                    .table(Questions::Table)
                    .col(Questions::AgentId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on asked_at for time-based queries and expiration cleanup.
        manager
            .create_index(
                Index::create()
                    .name("idx_questions_asked_at")
                    .table(Questions::Table)
                    .col(Questions::AskedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Questions::Table).to_owned()).await
    }
}

/// Iden enum matching the new `questions` table columns.
#[derive(DeriveIden)]
enum Questions {
    Table,
    Id,
    AgentId,
    WorkflowId,
    DispatchId,
    Category,
    Question,
    Context,
    Priority,
    Status,
    Answer,
    AskedAt,
    AnsweredAt,
    ExpiresAt,
}
