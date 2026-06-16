//! SeaORM migrations for the agentd-knowledge service.

use sea_orm_migration::prelude::*;

pub mod m20260613_000001_create_documents;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260613_000001_create_documents::Migration)]
    }
}
