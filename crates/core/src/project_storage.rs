//! SeaORM-based storage for project records.
//!
//! [`ProjectStorage`] provides CRUD operations for the `projects` table.
//! It holds a [`DatabaseConnection`] shared with the parent
//! [`crate::storage::Storage`].
//!
//! ## Tenant-scoped listing
//!
//! [`ProjectStorage::list_org`] includes rows with a `NULL` `organization_id`
//! alongside rows that match the requested org.  This preserves visibility of
//! legacy data created before multi-tenancy was introduced; those rows should
//! be backfilled by the `backfill-projects` admin command before NULL inclusion
//! is removed.

use anyhow::{anyhow, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::entity::project::{self, Column};

/// Storage operations for the `projects` table.
#[derive(Clone)]
pub struct ProjectStorage {
    db: DatabaseConnection,
}

impl ProjectStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Insert a new project and return the created record.
    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        organization_id: Option<&str>,
    ) -> Result<project::Model> {
        let now = chrono::Utc::now().to_rfc3339();
        let model = project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            organization_id: Set(organization_id.map(|s| s.to_string())),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(model)
    }

    /// Return the project with the given id, or `None` if not found.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<project::Model>> {
        Ok(project::Entity::find_by_id(id).one(&self.db).await?)
    }

    /// Return the project with the given name, or `None` if not found.
    pub async fn get_by_name(&self, name: &str) -> Result<Option<project::Model>> {
        Ok(project::Entity::find().filter(Column::Name.eq(name)).one(&self.db).await?)
    }

    /// Return all projects ordered by `created_at` descending.
    pub async fn list(&self) -> Result<Vec<project::Model>> {
        Ok(project::Entity::find().order_by_desc(Column::CreatedAt).all(&self.db).await?)
    }

    /// Return projects filtered by organization, or all projects when `org_id` is `None`.
    ///
    /// When `org_id` is provided, rows with a matching `organization_id` **or**
    /// a `NULL` `organization_id` are included.  The NULL-row inclusion is a
    /// deliberate tenant-transition aid: pre-migration data remains visible to
    /// authenticated tenants until the `backfill-projects` admin command is run.
    pub async fn list_org(&self, org_id: Option<&str>) -> Result<Vec<project::Model>> {
        let query = project::Entity::find().order_by_desc(Column::CreatedAt);
        let query = if let Some(oid) = org_id {
            // Include legacy NULL rows so pre-migration data is still visible
            // to authenticated tenants until backfill-tenant is run.
            query.filter(
                Condition::any()
                    .add(Column::OrganizationId.eq(oid))
                    .add(Column::OrganizationId.is_null()),
            )
        } else {
            query
        };
        Ok(query.all(&self.db).await?)
    }

    /// Return a paginated list of all projects ordered by `created_at` descending.
    ///
    /// Intended for product-admin use — not tenant-scoped.
    pub async fn list_paginated(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<agentd_common::types::PaginatedResponse<project::Model>> {
        let paginator =
            project::Entity::find().order_by_desc(Column::CreatedAt).paginate(&self.db, limit);
        let total = paginator.num_items().await?;
        let page = offset.checked_div(limit).unwrap_or(0);
        let items = paginator.fetch_page(page).await?;
        Ok(agentd_common::types::PaginatedResponse {
            items,
            total: total as usize,
            limit: limit as usize,
            offset: offset as usize,
        })
    }

    /// Update name and/or description for the given project.
    ///
    /// Returns an error if the project does not exist.
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<project::Model> {
        let proj = project::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("project not found: {}", id))?;

        let mut active: project::ActiveModel = proj.into();
        if let Some(name) = name {
            active.name = Set(name.to_string());
        }
        if let Some(description) = description {
            active.description = Set(Some(description.to_string()));
        }
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());
        Ok(active.update(&self.db).await?)
    }

    /// Delete the project with the given id.
    ///
    /// Returns `true` if a row was deleted, `false` if not found.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = project::Entity::delete_by_id(id).exec(&self.db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm_migration::MigratorTrait;

    async fn setup() -> (ProjectStorage, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (ProjectStorage::new(conn), tmp)
    }

    #[tokio::test]
    async fn test_create_and_get_by_id() {
        let (storage, _tmp) = setup().await;
        let proj = storage.create("My Project", Some("A test project"), None).await.unwrap();
        assert_eq!(proj.name, "My Project");
        assert_eq!(proj.description.as_deref(), Some("A test project"));
        assert!(proj.organization_id.is_none());

        let found = storage.get_by_id(&proj.id).await.unwrap().unwrap();
        assert_eq!(found.id, proj.id);
        assert_eq!(found.name, "My Project");
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let (storage, _tmp) = setup().await;
        storage.create("Named Project", None, None).await.unwrap();

        let found = storage.get_by_name("Named Project").await.unwrap().unwrap();
        assert_eq!(found.name, "Named Project");

        let missing = storage.get_by_name("nope").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let (storage, _tmp) = setup().await;
        let missing = storage.get_by_id("nonexistent-id").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_list_ordered_by_created_at_desc() {
        let (storage, _tmp) = setup().await;
        storage.create("Alpha", None, None).await.unwrap();
        storage.create("Beta", None, None).await.unwrap();

        let projects = storage.list().await.unwrap();
        assert_eq!(projects.len(), 2);
        // Beta was created after Alpha so it should come first (desc order)
        assert_eq!(projects[0].name, "Beta");
        assert_eq!(projects[1].name, "Alpha");
    }

    #[tokio::test]
    async fn test_list_org_filters_by_org_id() {
        let (storage, _tmp) = setup().await;
        let org_a = "org-a";
        let org_b = "org-b";
        storage.create("Proj A", None, Some(org_a)).await.unwrap();
        storage.create("Proj B", None, Some(org_b)).await.unwrap();
        // Legacy row with no org
        storage.create("Legacy", None, None).await.unwrap();

        // list_org(Some(org_a)) should return Proj A and Legacy (NULL)
        let for_a = storage.list_org(Some(org_a)).await.unwrap();
        let names_a: Vec<&str> = for_a.iter().map(|p| p.name.as_str()).collect();
        assert!(names_a.contains(&"Proj A"), "expected Proj A in {:?}", names_a);
        assert!(names_a.contains(&"Legacy"), "expected Legacy (NULL row) in {:?}", names_a);
        assert!(!names_a.contains(&"Proj B"), "did not expect Proj B in {:?}", names_a);
    }

    #[tokio::test]
    async fn test_list_org_none_returns_all() {
        let (storage, _tmp) = setup().await;
        storage.create("P1", None, Some("org-x")).await.unwrap();
        storage.create("P2", None, None).await.unwrap();

        let all = storage.list_org(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_list_paginated() {
        let (storage, _tmp) = setup().await;
        for i in 0..5u32 {
            storage.create(&format!("Project {i}"), None, None).await.unwrap();
        }

        let page = storage.list_paginated(2, 0).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 5);
        assert_eq!(page.limit, 2);
        assert_eq!(page.offset, 0);

        let page2 = storage.list_paginated(2, 2).await.unwrap();
        assert_eq!(page2.items.len(), 2);
        assert_eq!(page2.offset, 2);
    }

    #[tokio::test]
    async fn test_update() {
        let (storage, _tmp) = setup().await;
        let proj = storage.create("Old Name", Some("old desc"), None).await.unwrap();

        let updated = storage.update(&proj.id, Some("New Name"), Some("new desc")).await.unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.description.as_deref(), Some("new desc"));
        assert!(updated.updated_at >= proj.updated_at);
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
        let proj = storage.create("To Delete", None, None).await.unwrap();

        let deleted = storage.delete(&proj.id).await.unwrap();
        assert!(deleted);

        let found = storage.get_by_id(&proj.id).await.unwrap();
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
        storage.create("Duplicate", None, None).await.unwrap();
        let result = storage.create("Duplicate", None, None).await;
        assert!(result.is_err(), "duplicate name should be rejected");
    }
}
