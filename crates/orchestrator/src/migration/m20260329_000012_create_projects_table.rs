//! Migration 12: create the `projects` table.
//!
//! Projects group agents, workflows, rooms, and experiments under a
//! named logical boundary.  Later migrations add `project_id` FK columns
//! to those tables (#828).

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
                    .col(ColumnDef::new(Projects::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Projects::Description).string().null())
                    .col(ColumnDef::new(Projects::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Projects::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Projects::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
