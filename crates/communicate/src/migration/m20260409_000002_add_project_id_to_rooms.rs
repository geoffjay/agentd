//! Migration 2: add nullable `project_id` column to `rooms`.
//!
//! Rooms can be associated with a project by convention — there is no foreign
//! key since rooms live in a separate database from the projects table.
//! All existing rows receive `NULL` project_id — this migration is non-breaking.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Rooms::Table)
                    .add_column(ColumnDef::new(Rooms::ProjectId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rooms_project_id")
                    .table(Rooms::Table)
                    .col(Rooms::ProjectId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop().name("idx_rooms_project_id").to_owned()).await?;

        // SQLite does not support DROP COLUMN.
        Ok(())
    }
}

#[derive(Iden)]
enum Rooms {
    Table,
    ProjectId,
}
