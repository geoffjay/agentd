//! Migration 4: create the `projects` table in the core service.
//!
//! Projects sit below organizations in the tenant hierarchy:
//! `org > project > {agents, workflows, rooms, docs}`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Projects::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Projects::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(ColumnDef::new(Projects::Description).string().null())
                    .col(ColumnDef::new(Projects::OrganizationId).string().null())
                    .col(ColumnDef::new(Projects::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Projects::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Unique index on name — project names must be globally unique.
        manager
            .create_index(
                Index::create()
                    .name("idx_projects_name")
                    .table(Projects::Table)
                    .col(Projects::Name)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Non-unique index on organization_id for tenant-scoped listing.
        manager
            .create_index(
                Index::create()
                    .name("idx_projects_organization_id")
                    .table(Projects::Table)
                    .col(Projects::OrganizationId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Projects::Table).to_owned()).await
    }
}

/// Iden enum for the `projects` table columns.
#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    Name,
    Description,
    OrganizationId,
    CreatedAt,
    UpdatedAt,
}
