//! Migration 13: add nullable `project_id` column to `agents` and `workflows`.
//!
//! Projects (#827) group agents and workflows under a named logical boundary.
//! All existing rows receive `NULL` project_id — this migration is non-breaking.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add project_id to agents
        manager
            .alter_table(
                Table::alter()
                    .table(Agents::Table)
                    .add_column(ColumnDef::new(Agents::ProjectId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_agents_project_id")
                    .table(Agents::Table)
                    .col(Agents::ProjectId)
                    .to_owned(),
            )
            .await?;

        // Add project_id to workflows
        manager
            .alter_table(
                Table::alter()
                    .table(Workflows::Table)
                    .add_column(ColumnDef::new(Workflows::ProjectId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_workflows_project_id")
                    .table(Workflows::Table)
                    .col(Workflows::ProjectId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_agents_project_id").to_owned()).await?;

        manager.drop_index(Index::drop().name("idx_workflows_project_id").to_owned()).await?;

        // SQLite does not support DROP COLUMN — nothing more to do.
        Ok(())
    }
}

#[derive(Iden)]
enum Agents {
    Table,
    ProjectId,
}

#[derive(Iden)]
enum Workflows {
    Table,
    ProjectId,
}
