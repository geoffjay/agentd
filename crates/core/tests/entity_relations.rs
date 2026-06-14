//! Integration tests for core crate entity relations.
//!
//! Exercises SeaORM entities, eager loading, many-to-many traversal via the
//! Membership junction table, unique constraint enforcement, and migration
//! idempotency/rollback.
//!
//! All tests use a temporary file-backed SQLite database so they run fully
//! isolated and leave no state on disk.

use agentd_common::storage::create_test_connection;
use agentd_core::{
    entity::{membership, organization, session, user},
    migration::Migrator,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait, QueryFilter, Set,
};
use sea_orm_migration::MigratorTrait;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn setup_db() -> (sea_orm::DatabaseConnection, TempDir) {
    let (conn, tmp) = create_test_connection().await;
    Migrator::up(&conn, None).await.unwrap();
    (conn, tmp)
}

async fn insert_user(db: &sea_orm::DatabaseConnection, email: &str) -> user::Model {
    let now = chrono::Utc::now().to_rfc3339();
    user::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
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

async fn insert_org(
    db: &sea_orm::DatabaseConnection,
    name: &str,
    slug: &str,
) -> organization::Model {
    let now = chrono::Utc::now().to_rfc3339();
    organization::ActiveModel {
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
    db: &sea_orm::DatabaseConnection,
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

async fn insert_session(db: &sea_orm::DatabaseConnection, user_id: &str) -> session::Model {
    let now = chrono::Utc::now();
    session::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        user_id: Set(user_id.to_string()),
        token_hash: Set(format!("hash_{}", uuid::Uuid::new_v4())),
        expires_at: Set((now + chrono::Duration::hours(1)).to_rfc3339()),
        created_at: Set(now.to_rfc3339()),
    }
    .insert(db)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_crud_user() {
    let (db, _tmp) = setup_db().await;

    // Create
    let user = insert_user(&db, "alice@example.com").await;
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.role, "user");

    // Read
    let found = user::Entity::find_by_id(&user.id).one(&db).await.unwrap().unwrap();
    assert_eq!(found.id, user.id);

    // Update
    let mut active = found.into_active_model();
    active.display_name = Set(Some("Alice".to_string()));
    let updated = active.save(&db).await.unwrap();
    assert_eq!(updated.display_name.unwrap(), Some("Alice".to_string()));

    // Delete
    user::Entity::delete_by_id(&user.id).exec(&db).await.unwrap();
    let gone = user::Entity::find_by_id(&user.id).one(&db).await.unwrap();
    assert!(gone.is_none());
}

// ---------------------------------------------------------------------------
// Relation loading
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_user_has_many_memberships() {
    let (db, _tmp) = setup_db().await;

    let user = insert_user(&db, "bob@example.com").await;
    let org1 = insert_org(&db, "Alpha Inc", "alpha-inc").await;
    let org2 = insert_org(&db, "Beta LLC", "beta-llc").await;
    insert_membership(&db, &user.id, &org1.id, "owner").await;
    insert_membership(&db, &user.id, &org2.id, "member").await;

    // find_related: load memberships from an already-fetched user model.
    let memberships = user.find_related(membership::Entity).all(&db).await.unwrap();
    assert_eq!(memberships.len(), 2);
    let mut roles: Vec<&str> = memberships.iter().map(|m| m.role.as_str()).collect();
    roles.sort_unstable();
    assert_eq!(roles, ["member", "owner"]);
}

#[tokio::test]
async fn test_many_to_many_user_organizations_from_user_side() {
    let (db, _tmp) = setup_db().await;

    let user = insert_user(&db, "carol@example.com").await;
    let org1 = insert_org(&db, "Org A", "org-a").await;
    let org2 = insert_org(&db, "Org B", "org-b").await;
    insert_membership(&db, &user.id, &org1.id, "admin").await;
    insert_membership(&db, &user.id, &org2.id, "member").await;

    // find_with_related: batch-load all users with their organizations.
    let users_with_orgs =
        user::Entity::find().find_with_related(organization::Entity).all(&db).await.unwrap();

    assert_eq!(users_with_orgs.len(), 1);
    let (found_user, orgs) = &users_with_orgs[0];
    assert_eq!(found_user.email, "carol@example.com");
    assert_eq!(orgs.len(), 2);
    let mut slugs: Vec<&str> = orgs.iter().map(|o| o.slug.as_str()).collect();
    slugs.sort_unstable();
    assert_eq!(slugs, ["org-a", "org-b"]);
}

#[tokio::test]
async fn test_many_to_many_organization_users_from_org_side() {
    let (db, _tmp) = setup_db().await;

    let user1 = insert_user(&db, "dan@example.com").await;
    let user2 = insert_user(&db, "eve@example.com").await;
    let org = insert_org(&db, "Shared Corp", "shared-corp").await;
    insert_membership(&db, &user1.id, &org.id, "owner").await;
    insert_membership(&db, &user2.id, &org.id, "member").await;

    // find_with_related: batch-load all orgs with their members.
    let orgs_with_users =
        organization::Entity::find().find_with_related(user::Entity).all(&db).await.unwrap();

    assert_eq!(orgs_with_users.len(), 1);
    let (found_org, users) = &orgs_with_users[0];
    assert_eq!(found_org.slug, "shared-corp");
    assert_eq!(users.len(), 2);
    let mut emails: Vec<&str> = users.iter().map(|u| u.email.as_str()).collect();
    emails.sort_unstable();
    assert_eq!(emails, ["dan@example.com", "eve@example.com"]);
}

#[tokio::test]
async fn test_session_belongs_to_user() {
    let (db, _tmp) = setup_db().await;

    let user = insert_user(&db, "frank@example.com").await;
    let sess = insert_session(&db, &user.id).await;

    // find_also_related: load session and its user in a single query.
    let result = session::Entity::find()
        .filter(session::Column::Id.eq(&sess.id))
        .find_also_related(user::Entity)
        .one(&db)
        .await
        .unwrap();

    let (found_session, found_user) = result.unwrap();
    assert_eq!(found_session.id, sess.id);
    let found_user = found_user.unwrap();
    assert_eq!(found_user.email, "frank@example.com");
}

// ---------------------------------------------------------------------------
// Unique constraint enforcement
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unique_email_constraint() {
    let (db, _tmp) = setup_db().await;

    insert_user(&db, "grace@example.com").await;

    // Inserting a second user with the same email must fail.
    let now = chrono::Utc::now().to_rfc3339();
    let result = user::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        username: Set(None),
        email: Set("grace@example.com".to_string()),
        password_hash: Set("other_hash".to_string()),
        display_name: Set(None),
        role: Set("user".to_string()),
        is_superuser: Set(false),
        active_organization_id: Set(None),
        created_at: Set(now.clone()),
        updated_at: Set(now),
    }
    .insert(&db)
    .await;

    assert!(result.is_err(), "duplicate email should be rejected");
}

#[tokio::test]
async fn test_unique_membership_constraint() {
    let (db, _tmp) = setup_db().await;

    let user = insert_user(&db, "henry@example.com").await;
    let org = insert_org(&db, "Unique Org", "unique-org").await;
    insert_membership(&db, &user.id, &org.id, "member").await;

    // Inserting a second membership with the same (user_id, org_id) must fail.
    let now = chrono::Utc::now().to_rfc3339();
    let result = membership::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        user_id: Set(user.id.clone()),
        organization_id: Set(org.id.clone()),
        role: Set("admin".to_string()),
        created_at: Set(now.clone()),
        updated_at: Set(now),
    }
    .insert(&db)
    .await;

    assert!(result.is_err(), "duplicate (user_id, org_id) membership should be rejected");
}

// ---------------------------------------------------------------------------
// Migration lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_migration_idempotent() {
    let (db, _tmp) = create_test_connection().await;

    Migrator::up(&db, None).await.unwrap();
    // Running up a second time with if_not_exists tables must not error.
    Migrator::up(&db, None).await.unwrap();

    // Verify tables are usable after double-up.
    let users = user::Entity::find().all(&db).await.unwrap();
    assert!(users.is_empty());
}

#[tokio::test]
async fn test_migration_down_and_up() {
    let (db, _tmp) = create_test_connection().await;

    Migrator::up(&db, None).await.unwrap();

    // Seed a user so we can verify the table is gone after down().
    insert_user(&db, "ida@example.com").await;

    // Roll back all migrations.
    Migrator::down(&db, None).await.unwrap();

    // Re-apply — tables must be recreated and empty.
    Migrator::up(&db, None).await.unwrap();
    let users = user::Entity::find().all(&db).await.unwrap();
    assert!(users.is_empty(), "tables should be empty after down+up cycle");
}
