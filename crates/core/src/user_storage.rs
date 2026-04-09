//! SeaORM-based storage for user records.
//!
//! [`UserStorage`] provides CRUD operations for the `users` table, including
//! argon2 password hashing on create and constant-time verification on lookup.

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::entity::user::{self, Column};
use agentd_common::types::PaginatedResponse;

/// Storage operations for the `users` table.
#[derive(Clone)]
pub struct UserStorage {
    db: DatabaseConnection,
}

impl UserStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Hash a plaintext password with argon2id.
    fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("password hashing failed: {}", e))?;
        Ok(hash.to_string())
    }

    /// Verify a plaintext password against a stored argon2 hash.
    pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
        let parsed = PasswordHash::new(hash).map_err(|e| anyhow!("invalid hash: {}", e))?;
        Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    }

    /// Insert a new user with a hashed password and return the created record.
    pub async fn create(
        &self,
        username: Option<&str>,
        email: &str,
        display_name: Option<&str>,
        password: &str,
        role: &str,
    ) -> Result<user::Model> {
        let now = chrono::Utc::now().to_rfc3339();
        let model = user::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            username: Set(username.map(str::to_string)),
            email: Set(email.to_string()),
            password_hash: Set(Self::hash_password(password)?),
            display_name: Set(display_name.map(str::to_string)),
            role: Set(role.to_string()),
            active_organization_id: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(model)
    }

    /// Return the user with the given id, or `None` if not found.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<user::Model>> {
        Ok(user::Entity::find_by_id(id).one(&self.db).await?)
    }

    /// Return the user with the given username, or `None` if not found.
    pub async fn get_by_username(&self, username: &str) -> Result<Option<user::Model>> {
        Ok(user::Entity::find().filter(Column::Username.eq(username)).one(&self.db).await?)
    }

    /// Return the user with the given email, or `None` if not found.
    pub async fn get_by_email(&self, email: &str) -> Result<Option<user::Model>> {
        Ok(user::Entity::find().filter(Column::Email.eq(email)).one(&self.db).await?)
    }

    /// Update mutable user fields. Fields set to `None` are left unchanged.
    ///
    /// Returns an error if the user does not exist.
    pub async fn update(
        &self,
        id: &str,
        username: Option<&str>,
        display_name: Option<&str>,
        role: Option<&str>,
    ) -> Result<user::Model> {
        let user = user::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("user not found: {}", id))?;

        let mut active: user::ActiveModel = user.into();
        if let Some(u) = username {
            active.username = Set(Some(u.to_string()));
        }
        if let Some(dn) = display_name {
            active.display_name = Set(Some(dn.to_string()));
        }
        if let Some(r) = role {
            active.role = Set(r.to_string());
        }
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());
        Ok(active.update(&self.db).await?)
    }

    /// Delete the user with the given id.
    ///
    /// Returns `true` if a row was deleted, `false` if not found.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = user::Entity::delete_by_id(id).exec(&self.db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Set the user's active organization.
    ///
    /// Pass `None` to clear the active organization.
    pub async fn set_active_organization(
        &self,
        id: &str,
        organization_id: Option<&str>,
    ) -> Result<user::Model> {
        let user = user::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("user not found: {}", id))?;

        let mut active: user::ActiveModel = user.into();
        active.active_organization_id = Set(organization_id.map(str::to_string));
        active.updated_at = Set(chrono::Utc::now().to_rfc3339());
        Ok(active.update(&self.db).await?)
    }

    /// Return a paginated list of users ordered by email.
    pub async fn list_paginated(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<PaginatedResponse<user::Model>> {
        let paginator = user::Entity::find().order_by_asc(Column::Email).paginate(&self.db, limit);

        let total = paginator.num_items().await?;
        let page = if limit > 0 { offset / limit } else { 0 };
        let items = paginator.fetch_page(page).await?;

        Ok(PaginatedResponse {
            items,
            total: total as usize,
            limit: limit as usize,
            offset: offset as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm_migration::MigratorTrait;

    async fn setup() -> (UserStorage, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (UserStorage::new(conn), tmp)
    }

    #[tokio::test]
    async fn test_create_and_get_by_id() {
        let (storage, _tmp) = setup().await;
        let user = storage
            .create(Some("alice"), "alice@example.com", Some("Alice"), "secret", "user")
            .await
            .unwrap();
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.username, Some("alice".to_string()));
        assert_ne!(user.password_hash, "secret");

        let found = storage.get_by_id(&user.id).await.unwrap().unwrap();
        assert_eq!(found.id, user.id);
    }

    #[tokio::test]
    async fn test_get_by_username() {
        let (storage, _tmp) = setup().await;
        storage.create(Some("bob"), "bob@example.com", None, "pass", "user").await.unwrap();

        let found = storage.get_by_username("bob").await.unwrap().unwrap();
        assert_eq!(found.email, "bob@example.com");

        let missing = storage.get_by_username("nobody").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_get_by_email() {
        let (storage, _tmp) = setup().await;
        storage.create(None, "carol@example.com", None, "pass", "user").await.unwrap();

        let found = storage.get_by_email("carol@example.com").await.unwrap().unwrap();
        assert_eq!(found.role, "user");

        let missing = storage.get_by_email("nope@example.com").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_password_hash_and_verify() {
        let (storage, _tmp) = setup().await;
        let user = storage
            .create(Some("dan"), "dan@example.com", None, "my_password", "user")
            .await
            .unwrap();

        assert!(UserStorage::verify_password("my_password", &user.password_hash).unwrap());
        assert!(!UserStorage::verify_password("wrong", &user.password_hash).unwrap());
    }

    #[tokio::test]
    async fn test_update() {
        let (storage, _tmp) = setup().await;
        let user =
            storage.create(Some("eve"), "eve@example.com", None, "pass", "user").await.unwrap();

        let updated =
            storage.update(&user.id, Some("eve2"), Some("Eve Smith"), Some("admin")).await.unwrap();
        assert_eq!(updated.username, Some("eve2".to_string()));
        assert_eq!(updated.display_name, Some("Eve Smith".to_string()));
        assert_eq!(updated.role, "admin");
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let (storage, _tmp) = setup().await;
        let result = storage.update("nonexistent", None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _tmp) = setup().await;
        let user =
            storage.create(Some("frank"), "frank@example.com", None, "pass", "user").await.unwrap();

        assert!(storage.delete(&user.id).await.unwrap());
        assert!(storage.get_by_id(&user.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let (storage, _tmp) = setup().await;
        assert!(!storage.delete("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_set_active_organization() {
        let (storage, _tmp) = setup().await;
        let user =
            storage.create(Some("grace"), "grace@example.com", None, "pass", "user").await.unwrap();
        assert!(user.active_organization_id.is_none());

        let org_id = Uuid::new_v4().to_string();
        let updated = storage.set_active_organization(&user.id, Some(&org_id)).await.unwrap();
        assert_eq!(updated.active_organization_id, Some(org_id));

        let cleared = storage.set_active_organization(&user.id, None).await.unwrap();
        assert!(cleared.active_organization_id.is_none());
    }

    #[tokio::test]
    async fn test_list_paginated() {
        let (storage, _tmp) = setup().await;
        for i in 0..5u8 {
            storage
                .create(
                    Some(&format!("user{i}")),
                    &format!("user{i}@example.com"),
                    None,
                    "pass",
                    "user",
                )
                .await
                .unwrap();
        }

        let page = storage.list_paginated(3, 0).await.unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.limit, 3);
        assert_eq!(page.offset, 0);

        let page2 = storage.list_paginated(3, 3).await.unwrap();
        assert_eq!(page2.items.len(), 2);
    }

    #[tokio::test]
    async fn test_unique_email_constraint() {
        let (storage, _tmp) = setup().await;
        storage.create(Some("henry"), "henry@example.com", None, "pass", "user").await.unwrap();
        let result =
            storage.create(Some("henry2"), "henry@example.com", None, "pass", "user").await;
        assert!(result.is_err(), "duplicate email should be rejected");
    }

    #[tokio::test]
    async fn test_unique_username_constraint() {
        let (storage, _tmp) = setup().await;
        storage.create(Some("ida"), "ida@example.com", None, "pass", "user").await.unwrap();
        let result = storage.create(Some("ida"), "ida2@example.com", None, "pass", "user").await;
        assert!(result.is_err(), "duplicate username should be rejected");
    }
}
