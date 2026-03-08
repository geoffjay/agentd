# Storage Patterns and Conventions

This document describes how agentd services persist data to SQLite using
[SeaORM](https://www.sea-ql.org/SeaORM/).  Read this before writing any
storage code.

## Table of Contents

- [Overview](#overview)
- [Module Layout](#module-layout)
- [Entity Files](#entity-files)
- [Migration Files](#migration-files)
- [Storage Structs](#storage-structs)
- [Common Operations](#common-operations)
  - [Insert with ActiveModel](#insert-with-activemodel)
  - [Find by ID](#find-by-id)
  - [Update Specific Fields](#update-specific-fields)
  - [Delete](#delete)
  - [Filtered List](#filtered-list)
  - [Paginated List with Filters](#paginated-list-with-filters)
- [agentd-common Integration](#agentd-common-integration)
- [Adding a New Table](#adding-a-new-table)
- [Modifying an Existing Table](#modifying-an-existing-table)
- [Type Mapping Conventions](#type-mapping-conventions)
  - [UUIDs](#uuids)
  - [Booleans](#booleans)
  - [Timestamps](#timestamps)
  - [Enums](#enums)
  - [JSON Columns](#json-columns)
- [Testing Storage Code](#testing-storage-code)
- [Common Pitfalls](#common-pitfalls)
- [xtask Commands](#xtask-commands)

---

## Overview

agentd uses **SeaORM 1.1** with the `sqlx-sqlite` + `runtime-tokio-rustls`
feature set.  Every service that persists state follows the same pattern:

1. **Entity** — a `DeriveEntityModel` struct that maps directly to a table row.
2. **Migration** — a `MigrationTrait` implementation that creates / alters tables.
3. **Storage struct** — a `Clone`-able wrapper around `DatabaseConnection` that
   exposes typed CRUD methods to the rest of the crate.

The workspace dependencies are declared once in the root `Cargo.toml`:

```toml
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
sea-orm-migration = { version = "1.1" }
```

---

## Module Layout

```
crates/<service>/src/
  entity/
    mod.rs                  # re-exports one sub-module per table
    <table_name>.rs         # one file per table
  migration/
    mod.rs                  # MigratorTrait impl — registers migrations in order
    m<YYYYMMDD>_<seq>_<name>.rs   # one file per migration
  storage.rs                # public Storage struct + CRUD methods
  lib.rs                    # declares `pub mod entity; pub(crate) mod migration;`
                            # also exposes apply_migrations_for_path() / migration_status_for_path()
```

### Naming conventions

| Thing | Convention | Example |
|---|---|---|
| Entity file | `<table_name>.rs` | `notification.rs` |
| Migration file | `m<YYYYMMDD>_<seq>_<description>.rs` | `m20250305_000001_create_notifications_table.rs` |
| Iden enum | Table name in PascalCase | `Notifications`, `Agents` |
| Index name | `idx_<table>_<column>` | `idx_agents_status` |
| Unique index | `uq_<table>_<columns>` | `uq_dispatch_workflow_source` |

---

## Entity Files

An entity file contains exactly three items:

1. `Model` — the column definitions (`DeriveEntityModel`).
2. `Relation` — foreign-key relations (`DeriveRelation`).
3. `impl ActiveModelBehavior for ActiveModel {}` — required boilerplate.

```rust
// crates/notify/src/entity/notification.rs
// See docs/storage.md for patterns.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    /// UUID stored as TEXT — primary key.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,

    pub source_type: String,
    pub source_data: String,        // JSON: serialized NotificationSource
    pub lifetime_type: String,      // "Ephemeral" | "Persistent"
    pub lifetime_expires_at: Option<String>, // RFC3339, None for Persistent
    pub priority: String,           // "Low" | "Normal" | "High" | "Urgent"
    pub status: String,             // "Pending" | "Viewed" | …
    pub title: String,
    pub message: String,
    pub requires_response: i32,     // boolean: 0 or 1
    pub response: Option<String>,
    pub created_at: String,         // RFC3339
    pub updated_at: String,         // RFC3339
}

// No foreign-key relations for this table.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

When a table **has relations** (e.g., `workflows` → `dispatch_log`):

```rust
// crates/orchestrator/src/entity/workflow.rs
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::dispatch::Entity")]
    Dispatch,
}

impl Related<super::dispatch::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Dispatch.def()
    }
}
```

---

## Migration Files

Each migration file implements `MigrationTrait` with an `up()` and `down()`.
Every `create_table` call uses `.if_not_exists()` for idempotency.

```rust
// crates/notify/src/migration/m20250305_000001_create_notifications_table.rs

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Notifications::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Notifications::Status).string().not_null())
                    .col(ColumnDef::new(Notifications::CreatedAt).string().not_null())
                    // … more columns …
                    .to_owned(),
            )
            .await?;

        // Add an index for common filter queries
        manager
            .create_index(
                Index::create()
                    .name("idx_status")
                    .table(Notifications::Table)
                    .col(Notifications::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Notifications::Table).to_owned()).await
    }
}

/// Column Iden enum — must list every column used in the migration.
#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    Status,
    CreatedAt,
    // …
}
```

The migration is registered in `migration/mod.rs`:

```rust
// crates/notify/src/migration/mod.rs

pub use sea_orm_migration::prelude::*;

mod m20250305_000001_create_notifications_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250305_000001_create_notifications_table::Migration),
            // New migrations appended here, in chronological order
        ]
    }
}
```

---

## Storage Structs

Every service exposes a `FooStorage` struct that:

- Holds a `DatabaseConnection` (which is `Clone + Send + Sync`).
- Runs migrations in the constructor (`with_path`).
- Provides typed CRUD methods that convert between entity `Model` and domain types.

```rust
#[derive(Clone)]
pub struct NotificationStorage {
    db: DatabaseConnection,
}

impl NotificationStorage {
    /// Platform-specific path: ~/.local/share/agentd-notify/notify.db (Linux)
    ///                         ~/Library/Application Support/agentd-notify/notify.db (macOS)
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-notify", "notify.db")
    }

    /// Creates storage at the default path, running migrations.
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path).await
    }

    /// Creates storage at an explicit path (used in tests).
    pub async fn with_path(db_path: &Path) -> Result<Self> {
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        Ok(Self { db })
    }
}
```

When two storage structs share the same database (e.g., `AgentStorage` and
`SchedulerStorage` in the orchestrator), pass the `DatabaseConnection` rather
than opening a second file:

```rust
// In orchestrator main.rs / service setup:
let agent_storage = AgentStorage::new().await?;
let scheduler_storage = SchedulerStorage::new(agent_storage.db().clone());
```

The `db()` accessor is:

```rust
pub fn db(&self) -> &DatabaseConnection {
    &self.db
}
```

---

## Common Operations

All examples use the `notify` crate's `notifications` table.

### Insert with ActiveModel

```rust
use sea_orm::{EntityTrait, Set};
use crate::entity::notification as notif_entity;

pub async fn add(&self, notification: &Notification) -> Result<Uuid> {
    let model = notif_entity::ActiveModel {
        id: Set(notification.id.to_string()),
        status: Set(format!("{:?}", notification.status)),  // enum → String
        requires_response: Set(if notification.requires_response { 1 } else { 0 }),
        source_data: Set(serde_json::to_string(&notification.source)?),  // JSON
        created_at: Set(notification.created_at.to_rfc3339()),
        updated_at: Set(notification.updated_at.to_rfc3339()),
        // … all other columns …
    };

    notif_entity::Entity::insert(model).exec(&self.db).await?;
    Ok(notification.id)
}
```

Every field must be wrapped in `Set(…)`.  Use `NotSet` only for columns that
have database-level defaults and you want to omit entirely.

### Find by ID

```rust
use sea_orm::EntityTrait;

pub async fn get(&self, id: &Uuid) -> Result<Option<Notification>> {
    let model = notif_entity::Entity::find_by_id(id.to_string())
        .one(&self.db)
        .await?;
    match model {
        Some(m) => Ok(Some(model_to_notification(m)?)),
        None => Ok(None),
    }
}
```

### Update Specific Fields

Use `update_many` + `col_expr` to update only the columns that changed.
Always check `rows_affected` to detect missing rows.

```rust
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::sea_query::Expr;

pub async fn update(&self, notification: &Notification) -> Result<()> {
    let result = notif_entity::Entity::update_many()
        .col_expr(
            notif_entity::Column::Status,
            Expr::value(format!("{:?}", notification.status)),
        )
        .col_expr(
            notif_entity::Column::Response,
            Expr::value(notification.response.clone()),
        )
        .col_expr(
            notif_entity::Column::UpdatedAt,
            Expr::value(notification.updated_at.to_rfc3339()),
        )
        .filter(notif_entity::Column::Id.eq(notification.id.to_string()))
        .exec(&self.db)
        .await?;

    if result.rows_affected == 0 {
        anyhow::bail!("Notification not found");
    }
    Ok(())
}
```

### Delete

```rust
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub async fn delete(&self, id: &Uuid) -> Result<()> {
    let result = notif_entity::Entity::delete_many()
        .filter(notif_entity::Column::Id.eq(id.to_string()))
        .exec(&self.db)
        .await?;

    if result.rows_affected == 0 {
        anyhow::bail!("Notification not found");
    }
    Ok(())
}
```

### Filtered List

```rust
use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};

pub async fn list(
    &self,
    status_filter: Option<NotificationStatus>,
) -> Result<Vec<Notification>> {
    let mut query = notif_entity::Entity::find()
        .order_by(notif_entity::Column::CreatedAt, Order::Desc);

    if let Some(status) = status_filter {
        query = query.filter(
            notif_entity::Column::Status.eq(format!("{:?}", status))
        );
    }

    let models = query.all(&self.db).await?;
    models.into_iter().map(model_to_notification).collect()
}
```

For OR conditions across multiple values, use `Condition::any()`:

```rust
use sea_orm::Condition;

let models = notif_entity::Entity::find()
    .filter(
        Condition::any()
            .add(notif_entity::Column::Status.eq("Pending"))
            .add(notif_entity::Column::Status.eq("Viewed")),
    )
    .all(&self.db)
    .await?;
```

### Paginated List with Filters

```rust
use sea_orm::{Condition, ColumnTrait, EntityTrait, Order, PaginatorTrait,
              QueryFilter, QueryOrder, QuerySelect};

pub async fn list_paginated(
    &self,
    status_filter: Option<NotificationStatus>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<Notification>, usize)> {
    let condition = match &status_filter {
        Some(s) => Condition::all()
            .add(notif_entity::Column::Status.eq(format!("{:?}", s))),
        None => Condition::all(),
    };

    // Count total matching rows first
    let total = notif_entity::Entity::find()
        .filter(condition.clone())
        .count(&self.db)
        .await? as usize;

    // Fetch the page
    let models = notif_entity::Entity::find()
        .filter(condition)
        .order_by(notif_entity::Column::CreatedAt, Order::Desc)
        .limit(limit as u64)
        .offset(offset as u64)
        .all(&self.db)
        .await?;

    let items = models.into_iter().map(model_to_notification).collect::<Result<Vec<_>>>()?;
    Ok((items, total))
}
```

---

## agentd-common Integration

The `agentd-common` crate (`crates/common`) exposes shared storage utilities
in `agentd_common::storage`:

| Function | Purpose |
|---|---|
| `get_db_path(project, filename)` | Returns the XDG/platform-specific path for the database file, creating parent directories. |
| `create_connection(path)` | Opens a `sqlite://…?mode=rwc` SeaORM `DatabaseConnection`. |
| `create_test_connection()` | Returns `(DatabaseConnection, TempDir)` for use in unit tests. |
| `apply_migrations::<M>(path)` | Opens a connection and runs `M::up(&db, None)`. |
| `migration_status::<M>(path)` | Returns `Vec<(migration_name, is_applied)>` for all registered migrations. |

Each service library also exposes two public wrappers in its `lib.rs` that
`cargo xtask migrate` and `cargo xtask migrate-status` call:

```rust
// crates/notify/src/lib.rs
pub async fn apply_migrations_for_path(db_path: &std::path::Path) -> anyhow::Result<()> {
    agentd_common::storage::apply_migrations::<migration::Migrator>(db_path).await
}

pub async fn migration_status_for_path(
    db_path: &std::path::Path,
) -> anyhow::Result<Vec<(String, bool)>> {
    agentd_common::storage::migration_status::<migration::Migrator>(db_path).await
}
```

---

## Adding a New Table

Follow these steps to add a `tags` table to the `notify` crate as an example.

### 1. Write the entity

```rust
// crates/notify/src/entity/tag.rs
// See docs/storage.md for patterns.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub notification_id: String,
    pub label: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

### 2. Re-export from entity/mod.rs

```rust
// crates/notify/src/entity/mod.rs
pub mod notification;
pub mod tag;      // ← add this
```

### 3. Write the migration

```rust
// crates/notify/src/migration/m20250401_000001_create_tags_table.rs

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tags::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Tags::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Tags::NotificationId).string().not_null())
                    .col(ColumnDef::new(Tags::Label).string().not_null())
                    .col(ColumnDef::new(Tags::CreatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tags_notification_id")
                    .table(Tags::Table)
                    .col(Tags::NotificationId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Tags::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Tags {
    Table,
    Id,
    NotificationId,
    Label,
    CreatedAt,
}
```

### 4. Register the migration

```rust
// crates/notify/src/migration/mod.rs
mod m20250305_000001_create_notifications_table;
mod m20250401_000001_create_tags_table;   // ← add this

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250305_000001_create_notifications_table::Migration),
            Box::new(m20250401_000001_create_tags_table::Migration),   // ← append
        ]
    }
}
```

**Important**: migrations are applied in the order returned by `migrations()`.
Always append new migrations at the end — never reorder existing entries.

### 5. Add storage methods and wire up in service code

Add methods to `NotificationStorage` (or a new `TagStorage` if the table
warrants its own struct) and update your API handlers.

---

## Modifying an Existing Table

Schema changes require a new migration file.  **Never edit an existing
migration** that has already been applied to any installation.

### Example: add a `tags` column to `notifications`

```rust
// crates/notify/src/migration/m20250501_000001_add_tags_to_notifications.rs

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Notifications::Table)
                    .add_column(
                        ColumnDef::new(Notifications::Tags)
                            .string()
                            .not_null()
                            .default("[]"),   // default for existing rows
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Notifications::Table)
                    .drop_column(Notifications::Tags)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Notifications {
    Table,
    Tags,
}
```

Then:
1. Add `pub tags: String` to `entity/notification.rs`.
2. Register the migration in `migration/mod.rs`.
3. Update `model_to_notification` and `add` / `update` in `storage.rs`.

---

## Type Mapping Conventions

SQLite has a flexible type system.  agentd uses the following conventions to
keep the mapping layer consistent.

### UUIDs

Store as `TEXT` (not `BLOB`).  The `uuid` crate's `to_string()` / `parse_str()`
methods handle conversion:

```rust
// Entity field
pub id: String,

// Writing
id: Set(my_uuid.to_string()),

// Reading
let id = Uuid::parse_str(&model.id)?;
```

### Booleans

SQLite has no native boolean type.  Store as `INTEGER` (`0` / `1`), map in
the domain conversion layer:

```rust
// Entity field
pub requires_response: i32,

// Writing
requires_response: Set(if value { 1 } else { 0 }),

// Reading
let requires_response = model.requires_response != 0;
```

In migrations, use `.integer().not_null().default(0)` for false-defaulting
booleans, or `.default(1)` for true-defaulting ones.

### Timestamps

Store as `TEXT` in RFC3339 format.  Use `chrono`:

```rust
// Entity field
pub created_at: String,

// Writing
created_at: Set(Utc::now().to_rfc3339()),

// Reading
let created_at = DateTime::parse_from_rfc3339(&model.created_at)?.with_timezone(&Utc);
```

### Enums

Map Rust enums to their `Debug` representation string.  The `parse()` method
on the domain type handles reading them back (implement `FromStr`):

```rust
// Writing
status: Set(format!("{:?}", agent.status)),   // "Pending", "Running", …

// Reading
let status: AgentStatus = model.status.parse()?;
```

For simple string-like enums without extra data, `{:?}` and `FromStr` keep the
mapping readable.  For enums with associated data (e.g., `NotificationSource`),
use JSON serialization (see below).

### JSON Columns

Complex types (nested structs, maps, enums with payloads) are serialized to
JSON and stored as `TEXT`.  Use `serde_json`:

```rust
// Entity field
pub source_data: String,   // JSON: serialized NotificationSource
pub env: String,           // JSON: HashMap<String, String>

// Writing
source_data: Set(serde_json::to_string(&notification.source)?),
env: Set(serde_json::to_string(&agent.config.env).unwrap_or_else(|_| "{}".to_string())),

// Reading
let source: NotificationSource = serde_json::from_str(&model.source_data)?;
let env: HashMap<String, String> = serde_json::from_str(&model.env).unwrap_or_default();
```

In migrations, add a sensible default for JSON columns:

```rust
.col(ColumnDef::new(Agents::Env).string().not_null().default("{}"))
.col(ColumnDef::new(Agents::ToolPolicy).string().not_null().default("{\"mode\":\"allow_all\"}"))
```

---

## Testing Storage Code

Use `agentd_common::storage::create_test_connection()` to get a temporary
in-memory-ish database for each test.  Keep the `TempDir` alive for the
duration of the test.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Helper: create isolated storage for each test
    async fn create_test_storage() -> (NotificationStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = NotificationStorage::with_path(&db_path).await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let (storage, _tmp) = create_test_storage().await;

        let notification = Notification::new(/* … */);
        let id = notification.id;

        storage.add(&notification).await.unwrap();
        let retrieved = storage.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, id);
    }

    #[tokio::test]
    async fn test_update() {
        let (storage, _tmp) = create_test_storage().await;
        let mut n = make_test_notification();
        storage.add(&n).await.unwrap();

        n.status = NotificationStatus::Viewed;
        n.updated_at = Utc::now();
        storage.update(&n).await.unwrap();

        let retrieved = storage.get(&n.id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, NotificationStatus::Viewed);
    }

    #[tokio::test]
    async fn test_delete_missing_returns_error() {
        let (storage, _tmp) = create_test_storage().await;
        let missing_id = Uuid::new_v4();
        assert!(storage.delete(&missing_id).await.is_err());
    }
}
```

**Key points:**

- Each test gets its own `TempDir` + database — tests never interfere with each other.
- `_tmp` keeps the `TempDir` alive; the database file is deleted when it is dropped.
- `with_path()` runs migrations automatically, so tests always see the current schema.
- Test helper functions (`create_test_storage`, `make_test_notification`) reduce
  boilerplate and make intent clear.

---

## Common Pitfalls

### Forgot `if_not_exists()` on `create_table`

Without `.if_not_exists()`, running migrations twice (e.g., in tests that share
a database) will error with "table already exists".  Always include it.

### Modified an existing migration

Once a migration has been applied, its `seaql_migrations` row records its
checksum.  Editing the file changes the checksum and causes SeaORM to refuse
to run or may leave the database in an inconsistent state.  **Create a new
migration file instead.**

### `rows_affected == 0` not checked

`update_many` and `delete_many` succeed even when no rows match the filter.
Check `result.rows_affected` to detect "not found" conditions and return an
appropriate error.

### Large result sets without pagination

Fetching all rows with `.all(&self.db)` on a table that may grow unboundedly
is a memory hazard.  Prefer `list_paginated` variants for API endpoints.

### UUID round-trip through `parse_str`

`Uuid::parse_str` is infallible only when the string is a valid UUID.  If a
non-UUID string ends up in the `id` column (e.g., from a bug or manual
database edit), it will return an error.  Handle this with `?` so the error
propagates rather than causing a panic.

### JSON deserialization failures

`serde_json::from_str` on a column that holds malformed JSON will fail.  Use
`.unwrap_or_default()` only when an empty/default value is safe (e.g.,
`HashMap<String, String>` for environment variables).  For required fields,
propagate the error with `?`.

### Sharing a connection vs. opening a second file

Opening two `DatabaseConnection` instances to the same SQLite file can cause
write conflicts.  When two storage structs belong to the same service, share
the connection:

```rust
let agent_storage = AgentStorage::new().await?;
let scheduler_storage = SchedulerStorage::new(agent_storage.db().clone());
```

---

## xtask Commands

Three `cargo xtask` sub-commands help manage databases during development:

```bash
# Apply all pending migrations for every service
cargo xtask migrate

# Apply migrations for a single service
cargo xtask migrate --service notify
cargo xtask migrate --service orchestrator

# Show migration status (applied / pending) for every service
cargo xtask migrate-status

# Regenerate entity files from the live database schema
# (requires sea-orm-cli: cargo install sea-orm-cli)
cargo xtask generate-entities
cargo xtask generate-entities --service notify
```

`migrate` and `migrate-status` work even when the database file does not yet
exist — they create it on the fly.  `generate-entities` requires the database
to exist (start the service once first, or run `migrate`).
