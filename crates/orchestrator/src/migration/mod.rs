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
mod m20250309_000002_add_usage_sessions;
mod m20250310_000003_rename_tmux_session;
mod m20250311_000004_add_network_policy;
mod m20250312_000005_add_docker_config;

/// The migration runner — applies all known migrations in order.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250305_000001_create_tables::Migration),
            Box::new(m20250309_000002_add_usage_sessions::Migration),
            Box::new(m20250310_000003_rename_tmux_session::Migration),
            Box::new(m20250311_000004_add_network_policy::Migration),
            Box::new(m20250312_000005_add_docker_config::Migration),
        ]
    }
}
