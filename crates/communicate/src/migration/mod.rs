pub use sea_orm_migration::prelude::*;

mod m20250319_000001_create_communicate_tables;
mod m20260409_000002_add_project_id_to_rooms;
mod m20260613_000003_add_organization_id;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250319_000001_create_communicate_tables::Migration),
            Box::new(m20260409_000002_add_project_id_to_rooms::Migration),
            Box::new(m20260613_000003_add_organization_id::Migration),
        ]
    }
}
