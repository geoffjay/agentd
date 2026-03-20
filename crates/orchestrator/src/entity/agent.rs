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
    pub session_id: Option<String>,
    /// JSON-serialized [`crate::types::ToolPolicy`].
    pub tool_policy: String,
    /// Execution backend type (e.g., "tmux", "docker"). Defaults to "tmux".
    pub backend_type: Option<String>,
    pub model: Option<String>,
    /// JSON-serialized `HashMap<String, String>`.
    pub env: String,
    pub created_at: String,
    pub updated_at: String,
    /// Optional token-count threshold that triggers an automatic context clear.
    /// Stored as nullable INTEGER; maps to `Option<u64>` in the domain layer.
    pub auto_clear_threshold: Option<i64>,
    /// Optional network policy for Docker containers (e.g., "internet", "isolated", "host_network").
    /// Stored as nullable TEXT; maps to `Option<NetworkPolicy>` in the domain layer.
    pub network_policy: Option<String>,
    /// Custom Docker image override for this agent. Nullable TEXT.
    pub docker_image: Option<String>,
    /// JSON-serialized `Vec<VolumeMount>` for additional Docker volume mounts.
    pub extra_mounts: Option<String>,
    /// JSON-serialized `ResourceLimits` (cpu_limit, memory_limit_mb).
    pub resource_limits: Option<String>,
    /// JSON-serialized `Vec<String>` of additional directory paths.
    /// Maps to Claude Code's `--add-dir` flag.
    pub additional_dirs: String,
    /// JSON-serialized `Vec<String>` of communicate room names.
    /// The agent is auto-joined to these rooms when it connects to the orchestrator.
    pub rooms: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
