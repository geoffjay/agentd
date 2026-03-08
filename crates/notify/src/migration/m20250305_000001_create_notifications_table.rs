//! Initial migration: create the `notifications` table with indexes.
//!
//! Matches the schema that was previously created by `init_schema()` so that
//! existing installations can adopt the migration framework without data loss.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create the notifications table (idempotent — skipped if already exists)
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Notifications::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Notifications::SourceType).string().not_null())
                    .col(ColumnDef::new(Notifications::SourceData).string().not_null())
                    .col(ColumnDef::new(Notifications::LifetimeType).string().not_null())
                    .col(ColumnDef::new(Notifications::LifetimeExpiresAt).string().null())
                    .col(ColumnDef::new(Notifications::Priority).string().not_null())
                    .col(ColumnDef::new(Notifications::Status).string().not_null())
                    .col(ColumnDef::new(Notifications::Title).string().not_null())
                    .col(ColumnDef::new(Notifications::Message).string().not_null())
                    .col(ColumnDef::new(Notifications::RequiresResponse).integer().not_null())
                    .col(ColumnDef::new(Notifications::Response).string().null())
                    .col(ColumnDef::new(Notifications::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Notifications::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Index on status for efficient status-filter queries
        manager
            .create_index(
                Index::create()
                    .name("idx_status")
                    .table(Notifications::Table)
                    .col(Notifications::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on created_at for sort-by-time queries
        manager
            .create_index(
                Index::create()
                    .name("idx_created_at")
                    .table(Notifications::Table)
                    .col(Notifications::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Notifications::Table).to_owned()).await
    }
}

/// Iden enum matching the `notifications` table columns.
#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    SourceType,
    SourceData,
    LifetimeType,
    LifetimeExpiresAt,
    Priority,
    Status,
    Title,
    Message,
    RequiresResponse,
    Response,
    CreatedAt,
    UpdatedAt,
}
