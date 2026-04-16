//! SeaORM-based storage for organization records.
//!
//! [`OrganizationStorage`] provides CRUD operations for the `organizations`
//! table. It holds a [`DatabaseConnection`] shared with the parent
//! [`crate::storage::Storage`].

use anyhow::{anyhow, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::entity::organization::{self, Column};

/// Storage operations for the `organizations` table.
#[derive(Clone)]
pub struct OrganizationStorage {
    db: DatabaseConnection,
}

impl OrganizationStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Insert a new organization and return the created record.
    pub async fn create(&self, name: &str, slug: &str) -> Result<organization::Model> {
        let now = chrono::Utc::now().to_rfc3339();
        let model = organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            slug: Set(slug.to_string()),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(model)
    }

    /// Return the organization with the given id, or `None` if not found.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<organization::Model>> {
        Ok(organization::Entity::find_by_id(id).one(&self.db).await?)
    }

    /// Return the organization with the given slug, or `None` if not found.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<organization::Model>> {
        Ok(organization::Entity::find().filter(Column::Slug.eq(slug)).one(&self.db).await?)
    }

    /// Return all organizations ordered by name.
    pub async fn list(&self) -> Result<Vec<organization::Model>> {
        Ok(organization::Entity::find().order_by_asc(Column::Name).all(&self.db).await?)
    }

    /// Update name and/or slug for the given organization.
    ///
    /// Returns an error if the organization does not exist.
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        slug: Option<&str>,
    ) -> Result<organization::Model> {
        let org = organization::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("organization not found: {}", id))?;

        let mut active: organization::ActiveModel = org.into();
        if let Some(name) = name {
            active.name = Set(name.to_string());
        }
        if let Some(slug) = slug {
            active.slug = Set(slug.to_string());
        }
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());
        Ok(active.update(&self.db).await?)
    }

    /// Delete the organization with the given id.
    ///
    /// Returns `true` if a row was deleted, `false` if not found.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = organization::Entity::delete_by_id(id).exec(&self.db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm_migration::MigratorTrait;

    async fn setup() -> (OrganizationStorage, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (OrganizationStorage::new(conn), tmp)
    }

    #[tokio::test]
    async fn test_create_and_get_by_id() {
        let (storage, _tmp) = setup().await;
        let org = storage.create("Acme Corp", "acme").await.unwrap();
        assert_eq!(org.name, "Acme Corp");
        assert_eq!(org.slug, "acme");

        let found = storage.get_by_id(&org.id).await.unwrap().unwrap();
        assert_eq!(found.id, org.id);
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let (storage, _tmp) = setup().await;
        storage.create("Builder Co", "builder").await.unwrap();

        let found = storage.get_by_slug("builder").await.unwrap().unwrap();
        assert_eq!(found.name, "Builder Co");

        let missing = storage.get_by_slug("nope").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_list_ordered_by_name() {
        let (storage, _tmp) = setup().await;
        storage.create("Zebra Inc", "zebra").await.unwrap();
        storage.create("Alpha LLC", "alpha").await.unwrap();

        let orgs = storage.list().await.unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].name, "Alpha LLC");
        assert_eq!(orgs[1].name, "Zebra Inc");
    }

    #[tokio::test]
    async fn test_update() {
        let (storage, _tmp) = setup().await;
        let org = storage.create("Old Name", "old-slug").await.unwrap();

        let updated = storage.update(&org.id, Some("New Name"), Some("new-slug")).await.unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.slug, "new-slug");
        assert!(updated.updated_at > org.updated_at);
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let (storage, _tmp) = setup().await;
        let result = storage.update("nonexistent-id", Some("x"), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _tmp) = setup().await;
        let org = storage.create("To Delete", "delete-me").await.unwrap();

        let deleted = storage.delete(&org.id).await.unwrap();
        assert!(deleted);

        let found = storage.get_by_id(&org.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let (storage, _tmp) = setup().await;
        let result = storage.delete("nonexistent-id").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_unique_name_constraint() {
        let (storage, _tmp) = setup().await;
        storage.create("Duplicate", "dup-1").await.unwrap();
        let result = storage.create("Duplicate", "dup-2").await;
        assert!(result.is_err(), "duplicate name should be rejected");
    }

    #[tokio::test]
    async fn test_unique_slug_constraint() {
        let (storage, _tmp) = setup().await;
        storage.create("First", "same-slug").await.unwrap();
        let result = storage.create("Second", "same-slug").await;
        assert!(result.is_err(), "duplicate slug should be rejected");
    }
}
