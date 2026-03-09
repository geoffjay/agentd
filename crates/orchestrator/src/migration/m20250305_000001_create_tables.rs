//! Initial migration: create the `agents`, `workflows`, and `dispatch_log` tables.
//!
//! Matches the schema previously managed by `init_schema()` calls in both
//! `AgentStorage` and `SchedulerStorage`, so existing installations can adopt
//! the migration framework without data loss.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // -----------------------------------------------------------------
        // agents table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Agents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Agents::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Agents::Name).string().not_null())
                    .col(ColumnDef::new(Agents::Status).string().not_null())
                    .col(ColumnDef::new(Agents::WorkingDir).string().not_null())
                    .col(ColumnDef::new(Agents::User).string().null())
                    .col(ColumnDef::new(Agents::Shell).string().not_null())
                    .col(ColumnDef::new(Agents::Interactive).integer().not_null().default(0))
                    .col(ColumnDef::new(Agents::Prompt).string().null())
                    .col(ColumnDef::new(Agents::Worktree).integer().not_null().default(0))
                    .col(ColumnDef::new(Agents::SystemPrompt).string().null())
                    .col(ColumnDef::new(Agents::TmuxSession).string().null())
                    .col(
                        ColumnDef::new(Agents::ToolPolicy)
                            .string()
                            .not_null()
                            .default("{\"mode\":\"allow_all\"}"),
                    )
                    .col(ColumnDef::new(Agents::Model).string().null())
                    .col(ColumnDef::new(Agents::Env).string().not_null().default("{}"))
                    .col(ColumnDef::new(Agents::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Agents::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agents_status")
                    .table(Agents::Table)
                    .col(Agents::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // -----------------------------------------------------------------
        // workflows table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Workflows::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Workflows::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Workflows::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Workflows::AgentId).string().not_null())
                    .col(ColumnDef::new(Workflows::SourceType).string().not_null())
                    .col(ColumnDef::new(Workflows::SourceConfig).string().not_null())
                    .col(ColumnDef::new(Workflows::PromptTemplate).string().not_null())
                    .col(
                        ColumnDef::new(Workflows::PollIntervalSecs)
                            .integer()
                            .not_null()
                            .default(60),
                    )
                    .col(ColumnDef::new(Workflows::Enabled).integer().not_null().default(1))
                    .col(
                        ColumnDef::new(Workflows::ToolPolicy)
                            .string()
                            .not_null()
                            .default("{\"mode\":\"allow_all\"}"),
                    )
                    .col(ColumnDef::new(Workflows::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Workflows::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // -----------------------------------------------------------------
        // dispatch_log table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(DispatchLog::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(DispatchLog::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(DispatchLog::WorkflowId).string().not_null())
                    .col(ColumnDef::new(DispatchLog::SourceId).string().not_null())
                    .col(ColumnDef::new(DispatchLog::AgentId).string().not_null())
                    .col(ColumnDef::new(DispatchLog::PromptSent).string().not_null())
                    .col(ColumnDef::new(DispatchLog::Status).string().not_null())
                    .col(ColumnDef::new(DispatchLog::DispatchedAt).string().not_null())
                    .col(ColumnDef::new(DispatchLog::CompletedAt).string().null())
                    .to_owned(),
            )
            .await?;

        // Composite unique constraint on (workflow_id, source_id)
        manager
            .create_index(
                Index::create()
                    .name("uq_dispatch_workflow_source")
                    .table(DispatchLog::Table)
                    .col(DispatchLog::WorkflowId)
                    .col(DispatchLog::SourceId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_dispatch_workflow")
                    .table(DispatchLog::Table)
                    .col(DispatchLog::WorkflowId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_dispatch_status")
                    .table(DispatchLog::Table)
                    .col(DispatchLog::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(DispatchLog::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Workflows::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Agents::Table).to_owned()).await?;
        Ok(())
    }
}

/// Iden enum for the `agents` table columns.
#[derive(DeriveIden)]
enum Agents {
    Table,
    Id,
    Name,
    Status,
    WorkingDir,
    User,
    Shell,
    Interactive,
    Prompt,
    Worktree,
    SystemPrompt,
    TmuxSession,
    ToolPolicy,
    Model,
    Env,
    CreatedAt,
    UpdatedAt,
}

/// Iden enum for the `workflows` table columns.
#[derive(DeriveIden)]
enum Workflows {
    Table,
    Id,
    Name,
    AgentId,
    SourceType,
    SourceConfig,
    PromptTemplate,
    PollIntervalSecs,
    Enabled,
    ToolPolicy,
    CreatedAt,
    UpdatedAt,
}

/// Iden enum for the `dispatch_log` table columns.
#[derive(DeriveIden)]
enum DispatchLog {
    Table,
    Id,
    WorkflowId,
    SourceId,
    AgentId,
    PromptSent,
    Status,
    DispatchedAt,
    CompletedAt,
}
