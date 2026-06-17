//! Migration: add `auth_provider` and `system_username` columns to `users`.
//!
//! `auth_provider` selects the authentication backend for a user: `"local"`
//! (argon2 password, the default) or `"pam"` (host PAM stack). It is added as
//! `NOT NULL DEFAULT 'local'` so every existing row stays on password auth.
//!
//! `system_username` is the immutable OS account name a `'pam'` user is
//! authenticated against. It is nullable (only `'pam'` users set it) and unique
//! (one app user per system account). SQLite treats multiple NULLs as distinct,
//! so a plain unique index permits any number of `'local'` users.

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
                    .add_column(
                        ColumnDef::new(Users::AuthProvider).string().not_null().default("local"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::SystemUsername).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_system_username")
                    .table(Users::Table)
                    .col(Users::SystemUsername)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not reliably support DROP COLUMN; rebuild the table
        // without the two new columns, preserving all others (matches the
        // pattern used by migrations 2 and 3).
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE users_backup AS SELECT id, username, email, password_hash, \
                 display_name, role, is_superuser, active_organization_id, created_at, \
                 updated_at FROM users",
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
    AuthProvider,
    SystemUsername,
}
