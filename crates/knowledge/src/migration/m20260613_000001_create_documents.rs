//! Initial migration: create `documents` table.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Documents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Documents::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Documents::ProjectId).string().not_null())
                    .col(ColumnDef::new(Documents::RelPath).string().not_null())
                    .col(ColumnDef::new(Documents::Title).string().not_null())
                    .col(ColumnDef::new(Documents::SizeBytes).big_integer().not_null())
                    .col(ColumnDef::new(Documents::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Documents::UpdatedAt).string().not_null())
                    .col(ColumnDef::new(Documents::OrganizationId).string().null())
                    .to_owned(),
            )
            .await?;

        // Index on project_id for efficient per-project listing.
        manager
            .create_index(
                Index::create()
                    .name("idx_documents_project_id")
                    .table(Documents::Table)
                    .col(Documents::ProjectId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one document per (project_id, rel_path).
        manager
            .create_index(
                Index::create()
                    .name("idx_documents_project_rel_path")
                    .table(Documents::Table)
                    .col(Documents::ProjectId)
                    .col(Documents::RelPath)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Secondary index for tenant scoping.
        manager
            .create_index(
                Index::create()
                    .name("idx_documents_org_project")
                    .table(Documents::Table)
                    .col(Documents::OrganizationId)
                    .col(Documents::ProjectId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Documents::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Documents {
    Table,
    Id,
    ProjectId,
    RelPath,
    Title,
    SizeBytes,
    CreatedAt,
    UpdatedAt,
    OrganizationId,
}
