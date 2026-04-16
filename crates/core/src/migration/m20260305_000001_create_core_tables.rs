//! Initial migration: create the `users`, `organizations`, `memberships`, and
//! `sessions` tables for the core service.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // -----------------------------------------------------------------
        // users table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Users::Email).string().not_null())
                    .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                    .col(ColumnDef::new(Users::DisplayName).string().null())
                    .col(ColumnDef::new(Users::Role).string().not_null().default("user"))
                    .col(ColumnDef::new(Users::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Users::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_email")
                    .table(Users::Table)
                    .col(Users::Email)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // -----------------------------------------------------------------
        // organizations table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Organizations::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Organizations::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Organizations::Name).string().not_null())
                    .col(ColumnDef::new(Organizations::Slug).string().not_null())
                    .col(ColumnDef::new(Organizations::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Organizations::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_organizations_name")
                    .table(Organizations::Table)
                    .col(Organizations::Name)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_organizations_slug")
                    .table(Organizations::Table)
                    .col(Organizations::Slug)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // -----------------------------------------------------------------
        // memberships table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Memberships::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Memberships::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Memberships::UserId).string().not_null())
                    .col(ColumnDef::new(Memberships::OrganizationId).string().not_null())
                    .col(ColumnDef::new(Memberships::Role).string().not_null().default("member"))
                    .col(ColumnDef::new(Memberships::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Memberships::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_membership_user_org")
                    .table(Memberships::Table)
                    .col(Memberships::UserId)
                    .col(Memberships::OrganizationId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // -----------------------------------------------------------------
        // sessions table
        // -----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::UserId).string().not_null())
                    .col(ColumnDef::new(Sessions::TokenHash).string().not_null())
                    .col(ColumnDef::new(Sessions::ExpiresAt).string().not_null())
                    .col(ColumnDef::new(Sessions::CreatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_token_hash")
                    .table(Sessions::Table)
                    .col(Sessions::TokenHash)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_expires_at")
                    .table(Sessions::Table)
                    .col(Sessions::ExpiresAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse dependency order: sessions and memberships first
        // (they reference users/organizations), then organizations, then users.
        manager.drop_table(Table::drop().table(Sessions::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Memberships::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Organizations::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await?;
        Ok(())
    }
}

/// Iden enum for the `users` table columns.
#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Email,
    PasswordHash,
    DisplayName,
    Role,
    CreatedAt,
    UpdatedAt,
}

/// Iden enum for the `organizations` table columns.
#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
    Name,
    Slug,
    CreatedAt,
    UpdatedAt,
}

/// Iden enum for the `memberships` table columns.
#[derive(DeriveIden)]
enum Memberships {
    Table,
    Id,
    UserId,
    OrganizationId,
    Role,
    CreatedAt,
    UpdatedAt,
}

/// Iden enum for the `sessions` table columns.
#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    UserId,
    TokenHash,
    ExpiresAt,
    CreatedAt,
}
