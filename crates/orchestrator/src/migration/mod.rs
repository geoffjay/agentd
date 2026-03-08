//! SeaORM migration runner for the orchestrator service.
//!
//! Run all pending migrations at service startup:
//!
//! ```rust,ignore
//! use orchestrator::migration::Migrator;
//! use sea_orm_migration::MigratorTrait;
//!
//! Migrator::up(&db, None).await?;
//! ```

pub use sea_orm_migration::prelude::*;

mod m20250305_000001_create_tables;

/// The migration runner — applies all known migrations in order.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20250305_000001_create_tables::Migration)]
    }
}
