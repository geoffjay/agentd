//! Migration: add `is_superuser` boolean column to `users`.
//!
//! `is_superuser` marks a **product-level** superuser — a user permitted to
//! access the product admin area (`/admin`) and view core entities across the
//! entire product. It is orthogonal to both `role` (product `"admin"`/`"user"`)
//! and organization membership roles (`owner`/`admin`/`member`).
//!
//! Added as `NOT NULL DEFAULT false` so existing rows become non-superusers.

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
                        ColumnDef::new(Users::IsSuperuser).boolean().not_null().default(false),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite does not reliably support DROP COLUMN; rebuild the table
        // without `is_superuser`, preserving all other columns.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE users_backup AS SELECT id, username, email, password_hash, \
                 display_name, role, active_organization_id, created_at, updated_at FROM users",
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
    IsSuperuser,
}
