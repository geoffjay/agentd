//! Initial migration: create the `questions` table with indexes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Questions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Questions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Questions::NotificationId).string().not_null())
                    .col(ColumnDef::new(Questions::CheckType).string().not_null())
                    .col(ColumnDef::new(Questions::AskedAt).string().not_null())
                    .col(ColumnDef::new(Questions::Status).string().not_null())
                    .col(ColumnDef::new(Questions::Answer).string().null())
                    .to_owned(),
            )
            .await?;

        // Index on status for filtering active questions
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

        // Index on asked_at for time-based queries and cleanup
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

/// Iden enum matching the `questions` table columns.
#[derive(DeriveIden)]
enum Questions {
    Table,
    Id,
    NotificationId,
    CheckType,
    AskedAt,
    Status,
    Answer,
}
