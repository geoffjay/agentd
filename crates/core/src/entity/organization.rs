//! SeaORM entity for the `organizations` table.
//!
//! Organizations and users share a many-to-many relationship via the
//! [`super::membership`] junction table. See the `via()` impls below for the
//! canonical agentd pattern for traversing junction-table relations.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `organizations` table.
///
/// Both `name` and `slug` carry unique indexes enforced by the migration.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    #[sea_orm(unique)]
    pub slug: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::membership::Entity")]
    Memberships,
}

impl Related<super::membership::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Memberships.def()
    }
}

/// Many-to-many: Organization → User via Membership junction table.
///
/// - `via()`: traverse the `Organization` side of the membership relation in
///   reverse (organization → membership)
/// - `to()`: then follow membership's `User` relation (membership → user)
impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        super::membership::Relation::User.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::membership::Relation::Organization.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{membership, user};
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    use sea_orm_migration::MigratorTrait;

    async fn setup_db() -> (DatabaseConnection, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (conn, tmp)
    }

    async fn insert_user(db: &DatabaseConnection, email: &str) -> user::Model {
        let now = chrono::Utc::now().to_rfc3339();
        user::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            username: Set(None),
            email: Set(email.to_string()),
            password_hash: Set("hash".to_string()),
            display_name: Set(None),
            role: Set("user".to_string()),
            is_superuser: Set(false),
            auth_provider: Set("local".to_string()),
            system_username: Set(None),
            active_organization_id: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    async fn insert_org(db: &DatabaseConnection, name: &str, slug: &str) -> Model {
        let now = chrono::Utc::now().to_rfc3339();
        ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            slug: Set(slug.to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    async fn insert_membership(
        db: &DatabaseConnection,
        user_id: &str,
        org_id: &str,
        role: &str,
    ) -> membership::Model {
        let now = chrono::Utc::now().to_rfc3339();
        membership::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            organization_id: Set(org_id.to_string()),
            role: Set(role.to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_org_insert_and_find() {
        let (db, _tmp) = setup_db().await;
        let org = insert_org(&db, "Acme Corp", "acme").await;

        let found = Entity::find_by_id(&org.id).one(&db).await.unwrap().unwrap();
        assert_eq!(found.name, "Acme Corp");
        assert_eq!(found.slug, "acme");
    }

    #[tokio::test]
    async fn test_membership_links_user_and_org() {
        let (db, _tmp) = setup_db().await;
        let user = insert_user(&db, "bob@example.com").await;
        let org = insert_org(&db, "Builder Inc", "builder").await;
        let mem = insert_membership(&db, &user.id, &org.id, "member").await;

        assert_eq!(mem.user_id, user.id);
        assert_eq!(mem.organization_id, org.id);
        assert_eq!(mem.role, "member");
    }

    #[tokio::test]
    async fn test_find_users_with_related_organizations() {
        let (db, _tmp) = setup_db().await;
        let user = insert_user(&db, "carol@example.com").await;
        let org1 = insert_org(&db, "Org Alpha", "org-alpha").await;
        let org2 = insert_org(&db, "Org Beta", "org-beta").await;
        insert_membership(&db, &user.id, &org1.id, "owner").await;
        insert_membership(&db, &user.id, &org2.id, "member").await;

        // Eager-load: find all users and their organizations via membership.
        let users_with_orgs =
            user::Entity::find().find_with_related(Entity).all(&db).await.unwrap();

        assert_eq!(users_with_orgs.len(), 1);
        let (found_user, orgs) = &users_with_orgs[0];
        assert_eq!(found_user.email, "carol@example.com");
        assert_eq!(orgs.len(), 2);
        let mut slugs: Vec<&str> = orgs.iter().map(|o| o.slug.as_str()).collect();
        slugs.sort_unstable();
        assert_eq!(slugs, ["org-alpha", "org-beta"]);
    }

    #[tokio::test]
    async fn test_find_org_with_related_users() {
        let (db, _tmp) = setup_db().await;
        let user1 = insert_user(&db, "dan@example.com").await;
        let user2 = insert_user(&db, "eve@example.com").await;
        let org = insert_org(&db, "Shared Org", "shared").await;
        insert_membership(&db, &user1.id, &org.id, "admin").await;
        insert_membership(&db, &user2.id, &org.id, "member").await;

        // Eager-load: find org and all its members.
        let orgs_with_users =
            Entity::find().find_with_related(user::Entity).all(&db).await.unwrap();

        assert_eq!(orgs_with_users.len(), 1);
        let (found_org, users) = &orgs_with_users[0];
        assert_eq!(found_org.slug, "shared");
        assert_eq!(users.len(), 2);
    }
}
