//! SeaORM-based storage for session records.
//!
//! [`SessionStorage`] manages bearer-token sessions for authenticated users.
//! Tokens are stored directly in the `token_hash` column (the raw random
//! 256-bit hex value is itself unguessable, so an additional hash layer adds
//! negligible security for session tokens). The `expires_at` field is an
//! RFC 3339 string compared lexicographically.

use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::entity::session;

/// Storage operations for the `sessions` table.
#[derive(Clone)]
pub struct SessionStorage {
    db: DatabaseConnection,
}

impl SessionStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Generate a cryptographically random 256-bit hex token.
    pub fn generate_token() -> String {
        use rand::Rng;
        let bytes: [u8; 32] = rand::thread_rng().gen();
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Insert a new session row and return the created record.
    pub async fn create(
        &self,
        user_id: &str,
        token_hash: &str,
        expires_at: &str,
    ) -> Result<session::Model> {
        let now = chrono::Utc::now().to_rfc3339();
        let model = session::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            token_hash: Set(token_hash.to_string()),
            expires_at: Set(expires_at.to_string()),
            created_at: Set(now),
        }
        .insert(&self.db)
        .await?;
        Ok(model)
    }

    /// Return the session with the given token hash, or `None` if not found.
    pub async fn get_by_token_hash(&self, token_hash: &str) -> Result<Option<session::Model>> {
        Ok(session::Entity::find()
            .filter(session::Column::TokenHash.eq(token_hash))
            .one(&self.db)
            .await?)
    }

    /// Delete the session with the given token hash.
    ///
    /// Returns `true` if a row was deleted, `false` if not found.
    pub async fn delete_by_token_hash(&self, token_hash: &str) -> Result<bool> {
        let result = session::Entity::delete_many()
            .filter(session::Column::TokenHash.eq(token_hash))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Return a paginated list of ALL sessions across every user (product-wide),
    /// ordered by creation time (newest first). Intended for product-admin use.
    ///
    /// Note: rows include `token_hash` — callers MUST map to a response type that
    /// excludes it before returning to clients (it is the raw bearer token).
    pub async fn list_all_paginated(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<agentd_common::types::PaginatedResponse<session::Model>> {
        let paginator = session::Entity::find()
            .order_by_desc(session::Column::CreatedAt)
            .paginate(&self.db, limit);
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

    /// Delete all sessions that have passed their `expires_at` timestamp.
    ///
    /// Returns the number of rows deleted.
    pub async fn delete_expired(&self) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = session::Entity::delete_many()
            .filter(session::Column::ExpiresAt.lt(now))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Delete all sessions belonging to the given user.
    ///
    /// Used during login to clean up stale sessions.
    pub async fn delete_expired_for_user(&self, user_id: &str) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = session::Entity::delete_many()
            .filter(session::Column::UserId.eq(user_id))
            .filter(session::Column::ExpiresAt.lt(now))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected)
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

    async fn setup() -> (SessionStorage, sea_orm::DatabaseConnection, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        let storage = SessionStorage::new(conn.clone());
        (storage, conn, tmp)
    }

    async fn insert_user(db: &sea_orm::DatabaseConnection, email: &str) -> user::Model {
        let now = chrono::Utc::now().to_rfc3339();
        user::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            username: Set(None),
            email: Set(email.to_string()),
            password_hash: Set("hash".to_string()),
            display_name: Set(None),
            role: Set("user".to_string()),
            is_superuser: Set(false),
            active_organization_id: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .unwrap()
    }

    fn future_expiry() -> String {
        (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()
    }

    fn past_expiry() -> String {
        (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339()
    }

    #[tokio::test]
    async fn test_generate_token_is_64_hex_chars() {
        let token = SessionStorage::generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_generate_token_is_unique() {
        let t1 = SessionStorage::generate_token();
        let t2 = SessionStorage::generate_token();
        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "alice@example.com").await;
        let token = SessionStorage::generate_token();

        let sess = storage.create(&user.id, &token, &future_expiry()).await.unwrap();
        assert_eq!(sess.user_id, user.id);
        assert_eq!(sess.token_hash, token);

        let found = storage.get_by_token_hash(&token).await.unwrap().unwrap();
        assert_eq!(found.id, sess.id);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let (storage, _db, _tmp) = setup().await;
        let result = storage.get_by_token_hash("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_by_token_hash() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "bob@example.com").await;
        let token = SessionStorage::generate_token();
        storage.create(&user.id, &token, &future_expiry()).await.unwrap();

        assert!(storage.delete_by_token_hash(&token).await.unwrap());
        assert!(storage.get_by_token_hash(&token).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_by_token_hash_not_found() {
        let (storage, _db, _tmp) = setup().await;
        assert!(!storage.delete_by_token_hash("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_expired() {
        let (storage, db, _tmp) = setup().await;
        let user = insert_user(&db, "carol@example.com").await;

        storage.create(&user.id, &SessionStorage::generate_token(), &past_expiry()).await.unwrap();
        storage.create(&user.id, &SessionStorage::generate_token(), &past_expiry()).await.unwrap();
        let live_token = SessionStorage::generate_token();
        storage.create(&user.id, &live_token, &future_expiry()).await.unwrap();

        let deleted = storage.delete_expired().await.unwrap();
        assert_eq!(deleted, 2);

        let remaining = storage.get_by_token_hash(&live_token).await.unwrap();
        assert!(remaining.is_some());
    }

    #[tokio::test]
    async fn test_delete_expired_for_user() {
        let (storage, db, _tmp) = setup().await;
        let u1 = insert_user(&db, "dan@example.com").await;
        let u2 = insert_user(&db, "eve@example.com").await;

        storage.create(&u1.id, &SessionStorage::generate_token(), &past_expiry()).await.unwrap();
        let live_token = SessionStorage::generate_token();
        storage.create(&u2.id, &live_token, &past_expiry()).await.unwrap();

        let deleted = storage.delete_expired_for_user(&u1.id).await.unwrap();
        assert_eq!(deleted, 1);

        // u2's expired session is untouched
        let u2_sess = storage.get_by_token_hash(&live_token).await.unwrap();
        assert!(u2_sess.is_some());
    }
}
