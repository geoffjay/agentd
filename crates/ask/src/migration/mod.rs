//! SeaORM migration runner for the ask service.
//!
//! Run all pending migrations at service startup:
//!
//! ```rust,ignore
//! use ask::migration::Migrator;
//! use sea_orm_migration::MigratorTrait;
//!
//! Migrator::up(&db, None).await?;
//! ```

pub use sea_orm_migration::prelude::*;

mod m20250328_000001_create_questions_table;
mod m20250401_000001_redesign_questions_table;
mod m20260613_000003_add_organization_id;

/// The migration runner — applies all known migrations in order.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250328_000001_create_questions_table::Migration),
            Box::new(m20250401_000001_redesign_questions_table::Migration),
            Box::new(m20260613_000003_add_organization_id::Migration),
        ]
    }
}
