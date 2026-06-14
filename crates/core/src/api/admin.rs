//! Product-admin endpoints (superuser only).
//!
//! These endpoints provide **read-only**, **product-wide** views of the core
//! entities (users, organizations, memberships, sessions) across *every*
//! organization. They intentionally do **not** apply tenant scoping — a
//! superuser sees everything. Access is enforced by the [`SuperUser`] extractor
//! on every handler (HTTP 403 for non-superusers).
//!
//! Sensitive fields are never serialized: user `password_hash` and session
//! `token_hash` (the raw bearer token) are omitted from all responses.
//!
//! | Method | Path                          | Description                  |
//! |--------|-------------------------------|------------------------------|
//! | GET    | `/api/v1/admin/users`         | All users (paginated)        |
//! | GET    | `/api/v1/admin/organizations` | All organizations (paginated)|
//! | GET    | `/api/v1/admin/memberships`   | All memberships (paginated)  |
//! | GET    | `/api/v1/admin/sessions`      | All sessions (paginated)     |

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agentd_common::{
    error::ApiError,
    types::{clamp_limit, PaginatedResponse},
};

use crate::{
    entity::{membership, session},
    middleware::admin::SuperUser,
};

use super::{
    auth::{OrgResponse, UserResponse},
    AppState,
};

// ---------------------------------------------------------------------------
// Query / response types
// ---------------------------------------------------------------------------

/// Standard `?limit=&offset=` pagination query.
#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

fn paginate_args(p: &Pagination) -> (u64, u64) {
    (clamp_limit(p.limit) as u64, p.offset.unwrap_or(0) as u64)
}

/// Admin view of a membership row.
#[derive(Debug, Serialize)]
pub struct MembershipResponse {
    pub id: String,
    pub user_id: String,
    pub organization_id: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<membership::Model> for MembershipResponse {
    fn from(m: membership::Model) -> Self {
        Self {
            id: m.id,
            user_id: m.user_id,
            organization_id: m.organization_id,
            role: m.role,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

/// Admin view of a session — **never** includes `token_hash` (the raw token).
#[derive(Debug, Serialize)]
pub struct SessionAdminResponse {
    pub id: String,
    pub user_id: String,
    pub expires_at: String,
    pub is_expired: bool,
    pub created_at: String,
}

impl From<session::Model> for SessionAdminResponse {
    fn from(s: session::Model) -> Self {
        let is_expired = s.expires_at <= chrono::Utc::now().to_rfc3339();
        Self {
            id: s.id,
            user_id: s.user_id,
            expires_at: s.expires_at,
            is_expired,
            created_at: s.created_at,
        }
    }
}

/// Map a `PaginatedResponse<M>` into a `PaginatedResponse<R>` preserving the page metadata.
fn map_page<M, R: From<M>>(page: PaginatedResponse<M>) -> PaginatedResponse<R> {
    PaginatedResponse {
        items: page.items.into_iter().map(R::from).collect(),
        total: page.total,
        limit: page.limit,
        offset: page.offset,
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users_handler))
        .route("/organizations", get(list_organizations_handler))
        .route("/memberships", get(list_memberships_handler))
        .route("/sessions", get(list_sessions_handler))
}

// ---------------------------------------------------------------------------
// Handlers (gated by the SuperUser extractor — 403 for non-superusers)
// ---------------------------------------------------------------------------

async fn list_users_handler(
    _su: SuperUser,
    State(state): State<AppState>,
    Query(p): Query<Pagination>,
) -> Result<impl IntoResponse, ApiError> {
    let (limit, offset) = paginate_args(&p);
    let page =
        state.storage.users().list_paginated(limit, offset).await.map_err(ApiError::Internal)?;
    Ok(Json(map_page::<_, UserResponse>(page)))
}

async fn list_organizations_handler(
    _su: SuperUser,
    State(state): State<AppState>,
    Query(p): Query<Pagination>,
) -> Result<impl IntoResponse, ApiError> {
    let (limit, offset) = paginate_args(&p);
    let page = state
        .storage
        .organizations()
        .list_paginated(limit, offset)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(map_page::<_, OrgResponse>(page)))
}

async fn list_memberships_handler(
    _su: SuperUser,
    State(state): State<AppState>,
    Query(p): Query<Pagination>,
) -> Result<impl IntoResponse, ApiError> {
    let (limit, offset) = paginate_args(&p);
    let page = state
        .storage
        .memberships()
        .list_all_paginated(limit, offset)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(map_page::<_, MembershipResponse>(page)))
}

async fn list_sessions_handler(
    _su: SuperUser,
    State(state): State<AppState>,
    Query(p): Query<Pagination>,
) -> Result<impl IntoResponse, ApiError> {
    let (limit, offset) = paginate_args(&p);
    let page = state
        .storage
        .sessions()
        .list_all_paginated(limit, offset)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(map_page::<_, SessionAdminResponse>(page)))
}

#[cfg(test)]
mod tests {
    use crate::{
        api::{create_router, AppState},
        session_storage::SessionStorage,
        storage::Storage,
    };
    use agentd_common::storage::create_test_connection;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn setup() -> (Router, Storage, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        let storage = Storage::new(conn).await.unwrap();
        let router = create_router(AppState { storage: storage.clone() });
        (router, storage, tmp)
    }

    /// Create a user (optionally a superuser) with an active session and return
    /// the bearer token.
    async fn user_with_token(storage: &Storage, email: &str, superuser: bool) -> String {
        let user = storage.users().create(Some(email), email, None, "pw", "user").await.unwrap();
        if superuser {
            storage.users().set_superuser(&user.id, true).await.unwrap();
        }
        let token = SessionStorage::generate_token();
        let expires = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        storage.sessions().create(&user.id, &token, &expires).await.unwrap();
        token
    }

    async fn get(app: Router, uri: &str, token: Option<&str>) -> axum::http::Response<Body> {
        let mut builder = Request::builder().uri(uri);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        app.oneshot(builder.body(Body::empty()).unwrap()).await.unwrap()
    }

    #[tokio::test]
    async fn admin_users_requires_auth() {
        let (app, _storage, _tmp) = setup().await;
        let resp = get(app, "/api/v1/admin/users", None).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_users_forbidden_for_non_superuser() {
        let (app, storage, _tmp) = setup().await;
        let token = user_with_token(&storage, "normal@example.com", false).await;
        let resp = get(app, "/api/v1/admin/users", Some(&token)).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_users_ok_for_superuser_and_omits_password_hash() {
        let (app, storage, _tmp) = setup().await;
        let token = user_with_token(&storage, "root@example.com", true).await;
        let resp = get(app, "/api/v1/admin/users", Some(&token)).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(!body["items"].as_array().unwrap().is_empty());

        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!text.contains("password_hash"), "admin users must not expose password_hash");
    }

    #[tokio::test]
    async fn admin_sessions_never_exposes_token() {
        let (app, storage, _tmp) = setup().await;
        let token = user_with_token(&storage, "root@example.com", true).await;
        let resp = get(app, "/api/v1/admin/sessions", Some(&token)).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(!text.contains("token_hash"), "must not expose the token_hash field");
        assert!(!text.contains(&token), "must not leak the raw session token value");
    }

    #[tokio::test]
    async fn admin_endpoints_are_product_wide() {
        let (app, storage, _tmp) = setup().await;
        // Two users in different (default personal) contexts; superuser should see both.
        let _ = user_with_token(&storage, "alice@example.com", false).await;
        let token = user_with_token(&storage, "super@example.com", true).await;
        let resp = get(app, "/api/v1/admin/users", Some(&token)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            body["total"].as_u64().unwrap() >= 2,
            "superuser should see all users across the product"
        );
    }
}
