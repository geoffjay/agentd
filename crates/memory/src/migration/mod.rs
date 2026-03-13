//! SeaORM migration runner for the agentd-memory service.

pub use sea_orm_migration::prelude::*;

mod m20260313_000001_create_memory_entries;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260313_000001_create_memory_entries::Migration)]
    }
}
