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
mod m20250311_000006_add_additional_dirs;
mod m20250312_000005_add_docker_config;
mod m20260316_000007_rename_trigger_columns;
mod m20260319_000008_add_rooms_to_agents;
mod m20260323_000009_add_launch_command_to_agents;
mod m20260324_000010_add_system_prompt_fields_to_agents;
mod m20260328_000011_add_task_queue;
mod m20260411_000012_add_pid_to_agents;
mod m20260415_000013_add_builtin_to_agents;
mod m20260417_000014_create_projects_table;
mod m20260417_000015_add_project_id_to_agents_workflows;
mod m20260417_000016_add_conversation_events;
mod m20260513_000017_add_skills_to_agents;
mod m20260525_000018_add_conversation_event_seq;
mod m20260610_000019_add_task_json_to_dispatch_log;

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
            Box::new(m20250311_000006_add_additional_dirs::Migration),
            Box::new(m20260316_000007_rename_trigger_columns::Migration),
            Box::new(m20260319_000008_add_rooms_to_agents::Migration),
            Box::new(m20260323_000009_add_launch_command_to_agents::Migration),
            Box::new(m20260324_000010_add_system_prompt_fields_to_agents::Migration),
            Box::new(m20260328_000011_add_task_queue::Migration),
            Box::new(m20260411_000012_add_pid_to_agents::Migration),
            Box::new(m20260415_000013_add_builtin_to_agents::Migration),
            Box::new(m20260417_000014_create_projects_table::Migration),
            Box::new(m20260417_000015_add_project_id_to_agents_workflows::Migration),
            Box::new(m20260417_000016_add_conversation_events::Migration),
            Box::new(m20260513_000017_add_skills_to_agents::Migration),
            Box::new(m20260525_000018_add_conversation_event_seq::Migration),
            Box::new(m20260610_000019_add_task_json_to_dispatch_log::Migration),
        ]
    }
}
