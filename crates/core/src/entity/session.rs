//! SeaORM entity for the `sessions` table.
//!
//! Sessions link a bearer token (stored as a hash) to a [`super::user`].
//! The `expires_at` field is an RFC 3339 string; queries compare it
//! lexicographically, which is correct for that format.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `sessions` table.
///
/// `token_hash` stores a hashed representation of the bearer token — never
/// the raw token. `expires_at` and `created_at` are RFC 3339 strings.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::user;
    use crate::migration::Migrator;
    use agentd_common::storage::create_test_connection;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    use sea_orm_migration::MigratorTrait;

    async fn setup_db() -> (DatabaseConnection, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        Migrator::up(&conn, None).await.unwrap();
        (conn, tmp)
    }

    fn make_user_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    async fn insert_user(db: &DatabaseConnection, user_id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        user::ActiveModel {
            id: Set(user_id.to_string()),
            username: Set(None),
            email: Set(format!("{}@example.com", &user_id[..8])),
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
        .unwrap();
    }

    #[tokio::test]
    async fn test_session_creation() {
        let (db, _tmp) = setup_db().await;
        let user_id = make_user_id();
        insert_user(&db, &user_id).await;

        let now = chrono::Utc::now();
        let expires = (now + chrono::Duration::hours(24)).to_rfc3339();
        let session = ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            user_id: Set(user_id.clone()),
            token_hash: Set("hashed_token_abc".to_string()),
            expires_at: Set(expires.clone()),
            created_at: Set(now.to_rfc3339()),
        };
        let model = session.insert(&db).await.unwrap();

        let found = Entity::find_by_id(&model.id).one(&db).await.unwrap().unwrap();
        assert_eq!(found.user_id, user_id);
        assert_eq!(found.token_hash, "hashed_token_abc");
        assert_eq!(found.expires_at, expires);
    }

    #[tokio::test]
    async fn test_session_validation_with_related_user() {
        let (db, _tmp) = setup_db().await;
        let user_id = make_user_id();
        insert_user(&db, &user_id).await;

        let now = chrono::Utc::now();
        let expires = (now + chrono::Duration::hours(1)).to_rfc3339();
        ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            user_id: Set(user_id.clone()),
            token_hash: Set("valid_token_hash".to_string()),
            expires_at: Set(expires.clone()),
            created_at: Set(now.to_rfc3339()),
        }
        .insert(&db)
        .await
        .unwrap();

        // Validate: find non-expired session by token hash, eager-load user.
        let result = Entity::find()
            .filter(Column::TokenHash.eq("valid_token_hash"))
            .filter(Column::ExpiresAt.gt(now.to_rfc3339()))
            .find_also_related(user::Entity)
            .one(&db)
            .await
            .unwrap();

        let (session, user) = result.unwrap();
        assert_eq!(session.token_hash, "valid_token_hash");
        let user = user.unwrap();
        assert_eq!(user.id, user_id);
    }

    #[tokio::test]
    async fn test_expired_session_cleanup() {
        let (db, _tmp) = setup_db().await;
        let user_id = make_user_id();
        insert_user(&db, &user_id).await;

        let now = chrono::Utc::now();
        let past = (now - chrono::Duration::hours(1)).to_rfc3339();
        let future = (now + chrono::Duration::hours(1)).to_rfc3339();

        // Insert one expired and one valid session.
        for (token, expires) in [("expired_hash", past.as_str()), ("valid_hash", future.as_str())] {
            ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                user_id: Set(user_id.clone()),
                token_hash: Set(token.to_string()),
                expires_at: Set(expires.to_string()),
                created_at: Set(now.to_rfc3339()),
            }
            .insert(&db)
            .await
            .unwrap();
        }

        // Cleanup expired sessions.
        Entity::delete_many()
            .filter(Column::ExpiresAt.lt(now.to_rfc3339()))
            .exec(&db)
            .await
            .unwrap();

        let remaining = Entity::find().all(&db).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].token_hash, "valid_hash");
    }
}
