//! Initial migration: create the `memory_entries` table with indexes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MemoryEntries::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(MemoryEntries::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(MemoryEntries::Content).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::MemoryType).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::Tags).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::CreatedBy).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::Owner).string().null())
                    .col(ColumnDef::new(MemoryEntries::Visibility).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::SharedWith).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::Refs).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::CreatedAt).string().not_null())
                    .col(ColumnDef::new(MemoryEntries::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Index on memory_type for type-filter queries
        manager
            .create_index(
                Index::create()
                    .name("idx_memory_entries_memory_type")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::MemoryType)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on created_by for actor-filter queries
        manager
            .create_index(
                Index::create()
                    .name("idx_memory_entries_created_by")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::CreatedBy)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on created_at for sort-by-time queries
        manager
            .create_index(
                Index::create()
                    .name("idx_memory_entries_created_at")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Index on visibility for visibility-filter queries
        manager
            .create_index(
                Index::create()
                    .name("idx_memory_entries_visibility")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::Visibility)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(MemoryEntries::Table).to_owned()).await
    }
}

/// Iden enum matching the `memory_entries` table columns.
#[derive(DeriveIden)]
enum MemoryEntries {
    Table,
    Id,
    Content,
    MemoryType,
    Tags,
    CreatedBy,
    Owner,
    Visibility,
    SharedWith,
    Refs,
    CreatedAt,
    UpdatedAt,
}
