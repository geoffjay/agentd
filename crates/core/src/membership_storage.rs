//! SeaORM-based storage for membership records.
//!
//! [`MembershipStorage`] manages the many-to-many relationship between users
//! and organizations through the `memberships` junction table. It holds a
//! [`DatabaseConnection`] shared with the parent [`crate::storage::Storage`].

use anyhow::{anyhow, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, Set,
};
use uuid::Uuid;

use crate::entity::{membership, organization};

/// Storage operations for the `memberships` junction table.
#[derive(Clone)]
pub struct MembershipStorage {
    db: DatabaseConnection,
}

impl MembershipStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Add a user to an organization with the given role.
    ///
    /// Returns an error if the `(user_id, org_id)` pair already exists
    /// (unique constraint on the junction table).
    pub async fn add_member(
        &self,
        user_id: &str,
        org_id: &str,
        role: &str,
    ) -> Result<membership::Model> {
        let now = chrono::Utc::now().to_rfc3339();
        let model = membership::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            organization_id: Set(org_id.to_string()),
            role: Set(role.to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(model)
    }

    /// Remove a user from an organization.
    ///
    /// Returns `true` if a membership row was deleted, `false` if not found.
    pub async fn remove_member(&self, user_id: &str, org_id: &str) -> Result<bool> {
        let mem = membership::Entity::find()
            .filter(membership::Column::UserId.eq(user_id))
            .filter(membership::Column::OrganizationId.eq(org_id))
            .one(&self.db)
            .await?;

        match mem {
            None => Ok(false),
            Some(m) => {
                m.delete(&self.db).await?;
                Ok(true)
            }
        }
    }

    /// Return all membership records for the given organization.
    pub async fn list_members(&self, org_id: &str) -> Result<Vec<membership::Model>> {
        Ok(membership::Entity::find()
            .filter(membership::Column::OrganizationId.eq(org_id))
            .all(&self.db)
            .await?)
    }

    /// Return all organizations a user belongs to.
    pub async fn list_user_organizations(&self, user_id: &str) -> Result<Vec<organization::Model>> {
        Ok(organization::Entity::find()
            .inner_join(membership::Entity)
            .filter(membership::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?)
    }

    /// Return the membership for `(user_id, org_id)`, or `None` if not found.
    pub async fn get_membership(
        &self,
        user_id: &str,
        org_id: &str,
    ) -> Result<Option<membership::Model>> {
        Ok(membership::Entity::find()
            .filter(membership::Column::UserId.eq(user_id))
            .filter(membership::Column::OrganizationId.eq(org_id))
            .one(&self.db)
            .await?)
    }

    /// Update the role of an existing membership.
    ///
    /// Returns an error if the membership does not exist.
    pub async fn update_role(
        &self,
        user_id: &str,
        org_id: &str,
        role: &str,
    ) -> Result<membership::Model> {
        let mem = membership::Entity::find()
            .filter(membership::Column::UserId.eq(user_id))
            .filter(membership::Column::OrganizationId.eq(org_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                anyhow!("membership not found for user {} in org {}", user_id, org_id)
            })?;

        let mut active: membership::ActiveModel = mem.into();
        active.role = Set(role.to_string());
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());
        Ok(active.update(&self.db).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::user;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm::{ActiveModelTrait, Set};
    use sea_orm_migration::MigratorTrait;

    async fn setup() -> (MembershipStorage, sea_orm::DatabaseConnection, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        let storage = MembershipStorage::new(conn.clone());
        (storage, conn, tmp)
    }

    async fn insert_user(db: &sea_orm::DatabaseConnection, email: &str) -> user::Model {
        let now = chrono::Utc::now().to_rfc3339();
        user::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            email: Set(email.to_string()),
            password_hash: Set("hash".to_string()),
            display_name: Set(None),
            role: Set("user".to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    async fn insert_org(
        db: &sea_orm::DatabaseConnection,
        name: &str,
        slug: &str,
    ) -> organization::Model {
        let now = chrono::Utc::now().to_rfc3339();
        organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            slug: Set(slug.to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_add_and_get_membership() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "alice@example.com").await;
        let org = insert_org(&db, "Org A", "org-a").await;

        let mem = storage.add_member(&user.id, &org.id, "owner").await.unwrap();
        assert_eq!(mem.user_id, user.id);
        assert_eq!(mem.organization_id, org.id);
        assert_eq!(mem.role, "owner");

        let found = storage.get_membership(&user.id, &org.id).await.unwrap().unwrap();
        assert_eq!(found.id, mem.id);
    }

    #[tokio::test]
    async fn test_remove_member() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "bob@example.com").await;
        let org = insert_org(&db, "Org B", "org-b").await;
        storage.add_member(&user.id, &org.id, "member").await.unwrap();

        let removed = storage.remove_member(&user.id, &org.id).await.unwrap();
        assert!(removed);

        let found = storage.get_membership(&user.id, &org.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_remove_member_not_found() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "carol@example.com").await;
        let org = insert_org(&db, "Org C", "org-c").await;

        let result = storage.remove_member(&user.id, &org.id).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_list_members() {
        let (storage, db, _tmp) = setup().await;
        let u1 = insert_user(&db, "dan@example.com").await;
        let u2 = insert_user(&db, "eve@example.com").await;
        let org = insert_org(&db, "Org D", "org-d").await;
        storage.add_member(&u1.id, &org.id, "owner").await.unwrap();
        storage.add_member(&u2.id, &org.id, "member").await.unwrap();

        let members = storage.list_members(&org.id).await.unwrap();
        assert_eq!(members.len(), 2);
    }

    #[tokio::test]
    async fn test_list_user_organizations() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "frank@example.com").await;
        let org1 = insert_org(&db, "Org E", "org-e").await;
        let org2 = insert_org(&db, "Org F", "org-f").await;
        storage.add_member(&user.id, &org1.id, "owner").await.unwrap();
        storage.add_member(&user.id, &org2.id, "member").await.unwrap();

        let orgs = storage.list_user_organizations(&user.id).await.unwrap();
        assert_eq!(orgs.len(), 2);
        let mut slugs: Vec<&str> = orgs.iter().map(|o| o.slug.as_str()).collect();
        slugs.sort_unstable();
        assert_eq!(slugs, ["org-e", "org-f"]);
    }

    #[tokio::test]
    async fn test_get_membership_not_found() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "grace@example.com").await;
        let org = insert_org(&db, "Org G", "org-g").await;

        let result = storage.get_membership(&user.id, &org.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_role() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "henry@example.com").await;
        let org = insert_org(&db, "Org H", "org-h").await;
        storage.add_member(&user.id, &org.id, "member").await.unwrap();

        let updated = storage.update_role(&user.id, &org.id, "admin").await.unwrap();
        assert_eq!(updated.role, "admin");
    }

    #[tokio::test]
    async fn test_duplicate_membership_rejected() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "ida@example.com").await;
        let org = insert_org(&db, "Org I", "org-i").await;
        storage.add_member(&user.id, &org.id, "member").await.unwrap();

        let result = storage.add_member(&user.id, &org.id, "owner").await;
        assert!(result.is_err(), "duplicate membership should be rejected");
    }
}
