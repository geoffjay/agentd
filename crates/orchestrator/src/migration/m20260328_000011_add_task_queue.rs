//! Migration 11: add the `task_queue` table for queue-based triggers.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TaskQueue::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TaskQueue::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(TaskQueue::QueueName).string().not_null())
                    .col(ColumnDef::new(TaskQueue::Title).string().not_null())
                    .col(ColumnDef::new(TaskQueue::Body).string().null())
                    .col(ColumnDef::new(TaskQueue::Priority).integer().not_null().default(0))
                    .col(ColumnDef::new(TaskQueue::Status).string().not_null().default("pending"))
                    .col(ColumnDef::new(TaskQueue::VisibilityTimeoutAt).string().null())
                    .col(ColumnDef::new(TaskQueue::RetryCount).integer().not_null().default(0))
                    .col(ColumnDef::new(TaskQueue::MaxRetries).integer().not_null().default(3))
                    .col(ColumnDef::new(TaskQueue::CreatedAt).string().not_null())
                    .col(ColumnDef::new(TaskQueue::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_task_queue_name_status")
                    .table(TaskQueue::Table)
                    .col(TaskQueue::QueueName)
                    .col(TaskQueue::Status)
                    .col(TaskQueue::Priority)
                    .col(TaskQueue::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(TaskQueue::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum TaskQueue {
    Table,
    Id,
    QueueName,
    Title,
    Body,
    Priority,
    Status,
    VisibilityTimeoutAt,
    RetryCount,
    MaxRetries,
    CreatedAt,
    UpdatedAt,
}
