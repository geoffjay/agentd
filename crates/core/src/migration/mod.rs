//! SeaORM migration runner for the core service.
//!
//! Run all pending migrations at service startup:
//!
//! ```rust,ignore
//! use agentd_core::migration::Migrator;
//! use sea_orm_migration::MigratorTrait;
//!
//! Migrator::up(&db, None).await?;
//! ```

pub use sea_orm_migration::prelude::*;

mod m20260305_000001_create_core_tables;
mod m20260408_000002_add_username_to_users;
mod m20260614_000003_add_is_superuser_to_users;
mod m20260616_000004_create_projects_table;

/// The migration runner — applies all known migrations in order.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260305_000001_create_core_tables::Migration),
            Box::new(m20260408_000002_add_username_to_users::Migration),
            Box::new(m20260614_000003_add_is_superuser_to_users::Migration),
            Box::new(m20260616_000004_create_projects_table::Migration),
        ]
    }
}
