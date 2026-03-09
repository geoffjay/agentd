//! SeaORM entity for the `agents` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `agents` table.
///
/// JSON columns (`tool_policy`, `env`) are stored as TEXT and deserialized
/// manually in the storage layer to preserve the existing serde representations.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "agents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub status: String,
    pub working_dir: String,
    pub user: Option<String>,
    pub shell: String,
    /// Stored as INTEGER (0/1); mapped to `bool` in the domain layer.
    pub interactive: i32,
    pub prompt: Option<String>,
    /// Stored as INTEGER (0/1); mapped to `bool` in the domain layer.
    pub worktree: i32,
    pub system_prompt: Option<String>,
    pub tmux_session: Option<String>,
    /// JSON-serialized [`crate::types::ToolPolicy`].
    pub tool_policy: String,
    pub model: Option<String>,
    /// JSON-serialized `HashMap<String, String>`.
    pub env: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
