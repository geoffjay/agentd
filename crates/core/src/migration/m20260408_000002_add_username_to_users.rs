//! Migration: add `username` and `active_organization_id` columns to `users`.
//!
//! `username` is a unique human-readable identifier used alongside email for
//! login. `active_organization_id` tracks the organization a user is currently
//! operating as, enabling per-request tenant isolation.
//!
//! Both columns are added as nullable so the migration is backwards-compatible
//! with any existing rows.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::Username).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::ActiveOrganizationId).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_username")
                    .table(Users::Table)
                    .col(Users::Username)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not support DROP COLUMN in older versions; use raw SQL
        // for the down migration to ensure compatibility.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE users_backup AS SELECT id, email, password_hash, \
                 display_name, role, created_at, updated_at FROM users",
            )
            .await?;
        manager.get_connection().execute_unprepared("DROP TABLE users").await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE users_backup RENAME TO users")
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Username,
    ActiveOrganizationId,
}
