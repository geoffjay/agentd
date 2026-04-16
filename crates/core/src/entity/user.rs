//! SeaORM entity for the `users` table.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `users` table.
///
/// IDs are UUID strings for SQLite compatibility. Timestamps are stored as
/// RFC 3339 strings. The `email` column has a unique index enforced by the
/// migration.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// Unique human-readable login identifier (added in migration 2).
    #[sea_orm(unique)]
    pub username: Option<String>,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    /// Role string — `"admin"` or `"user"`.
    pub role: String,
    /// The organization the user is currently operating as (nullable).
    pub active_organization_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::membership::Entity")]
    Memberships,
    #[sea_orm(has_many = "super::session::Entity")]
    Sessions,
}

impl Related<super::membership::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Memberships.def()
    }
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

/// Many-to-many: User → Organization via Membership junction table.
///
/// - `via()`: traverse the `User` side of the membership relation in reverse
///   (user → membership)
/// - `to()`: then follow membership's `Organization` relation
///   (membership → organization)
impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        super::membership::Relation::Organization.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::membership::Relation::User.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use sea_orm_migration::MigratorTrait;

    async fn setup_db() -> (DatabaseConnection, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (conn, tmp)
    }

    #[tokio::test]
    async fn test_user_insert_and_find() {
        let (db, _tmp) = setup_db().await;

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let user = ActiveModel {
            id: Set(id.clone()),
            username: Set(None),
            email: Set("alice@example.com".to_string()),
            password_hash: Set("hashed_password".to_string()),
            display_name: Set(Some("Alice".to_string())),
            role: Set("user".to_string()),
            active_organization_id: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        };
        let model = user.insert(&db).await.unwrap();

        let found = Entity::find_by_id(&model.id).one(&db).await.unwrap().unwrap();
        assert_eq!(found.id, id);
        assert_eq!(found.email, "alice@example.com");
        assert_eq!(found.display_name, Some("Alice".to_string()));
        assert_eq!(found.role, "user");
    }
}
