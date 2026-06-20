//! User profile endpoint handlers.
//!
//! # Endpoints (from issue #216 and #218)
//!
//! | Method | Path                                 | Auth | Description                             |
//! |--------|--------------------------------------|------|-----------------------------------------|
//! | GET    | `/api/v1/users/me`                   | Yes  | Get current user profile                |
//! | PUT    | `/api/v1/users/me`                   | Yes  | Update profile (display_name, email)    |
//! | PUT    | `/api/v1/users/me/password`          | Yes  | Change password (requires current pass) |
//! | GET    | `/api/v1/users/me/organizations`     | Yes  | List user's organizations               |
//! | PUT    | `/users/me/active-organization`      | Yes  | Switch active organization              |
//!
//! All endpoints require a valid `Authorization: Bearer <token>` header.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agentd_common::error::ApiError;

use crate::{api::auth::OrgResponse, middleware::auth::AuthUser, user_storage::UserStorage};

use super::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct SetActiveOrganizationRequest {
    pub organization_id: String,
}

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub id: String,
    pub username: Option<String>,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    pub active_organization_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<crate::entity::user::Model> for UserProfileResponse {
    fn from(u: crate::entity::user::Model) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            display_name: u.display_name,
            role: u.role,
            active_organization_id: u.active_organization_id,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Routes mounted at `/users` (legacy path, issue #216).
pub fn router() -> Router<AppState> {
    Router::new().route("/me/active-organization", put(set_active_organization_handler))
}

/// Routes mounted at `/api/v1/users` (issue #218).
pub fn v1_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_me_handler).put(update_profile_handler))
        .route("/me/password", put(change_password_handler))
        .route("/me/organizations", get(list_my_organizations_handler))
        .route("/me/active-organization", put(set_active_organization_handler))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/users/me`
///
/// Returns the current user's profile without `password_hash`.
async fn get_me_handler(auth: AuthUser) -> impl IntoResponse {
    Json(UserProfileResponse::from(auth.user))
}

/// `PUT /api/v1/users/me`
///
/// Updates `display_name` and/or `email`. Fields set to `null` or omitted are
/// left unchanged.
///
/// Returns `200 OK` with the updated profile.
async fn update_profile_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let users = state.storage.users();
    let mut user = auth.user;

    // Update display_name if provided
    if body.display_name.is_some() || body.email.is_some() {
        if let Some(dn) = &body.display_name {
            user = users
                .update(&user.id, None, Some(dn.as_str()), None)
                .await
                .map_err(ApiError::Internal)?;
        }
        if let Some(email) = &body.email {
            // Check email uniqueness before update
            if let Some(existing) = users.get_by_email(email).await.map_err(ApiError::Internal)? {
                if existing.id != user.id {
                    return Err(ApiError::Conflict(format!("email already in use: {email}")));
                }
            }
            user = users.update_email(&user.id, email).await.map_err(ApiError::Internal)?;
        }
    }

    Ok(Json(UserProfileResponse::from(user)))
}

/// `PUT /api/v1/users/me/password`
///
/// Changes the user's password. Requires the current password for verification.
///
/// Returns `204 No Content`.
async fn change_password_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.new_password.is_empty() {
        return Err(ApiError::InvalidInput("new password must not be empty".into()));
    }

    // Verify current password
    let ok = UserStorage::verify_password(&body.current_password, &auth.user.password_hash)
        .map_err(ApiError::Internal)?;
    if !ok {
        return Err(ApiError::Unauthorized("current password is incorrect".into()));
    }

    state
        .storage
        .users()
        .update_password(&auth.user.id, &body.new_password)
        .await
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/users/me/organizations`
///
/// Returns all organizations the current user belongs to.
async fn list_my_organizations_handler(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let orgs = state
        .storage
        .memberships()
        .list_user_organizations(&auth.user.id)
        .await
        .map_err(ApiError::Internal)?;

    let response: Vec<OrgResponse> = orgs.into_iter().map(OrgResponse::from).collect();
    Ok(Json(response))
}

/// `PUT /users/me/active-organization`
///
/// Switches the authenticated user's active organization. The user must be a
/// member of the specified organization; non-members receive `403 Forbidden`.
///
/// Returns `200 OK` with the updated user profile (no password hash).
async fn set_active_organization_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<SetActiveOrganizationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify membership — only members of the org may activate it.
    let membership = state
        .storage
        .memberships()
        .get_membership(&auth.user.id, &body.organization_id)
        .await
        .map_err(ApiError::Internal)?;

    if membership.is_none() {
        return Err(ApiError::Forbidden(format!(
            "user is not a member of organization {}",
            body.organization_id
        )));
    }

    // Update the user's active organization
    let updated = state
        .storage
        .users()
        .set_active_organization(&auth.user.id, Some(&body.organization_id))
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(UserProfileResponse::from(updated)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::api::AppState;
    use crate::storage::Storage;
    use agentd_common::storage::create_test_connection;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_app() -> (Router, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        let storage = Storage::new(conn).await.unwrap();
        let state = AppState::new(storage);
        let app = crate::api::create_router(state);
        (app, tmp)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn register(app: &Router, username: &str, email: &str) -> (String, String) {
        let payload =
            serde_json::json!({ "username": username, "email": email, "password": "testpass" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = body_json(response).await;
        let token = body["token"].as_str().unwrap().to_string();
        let user_id = body["user"]["id"].as_str().unwrap().to_string();
        (token, user_id)
    }

    // -----------------------------------------------------------------------
    // GET /api/v1/users/me
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_me() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "alice", "alice@example.com").await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/users/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["username"], "alice");
        assert_eq!(body["email"], "alice@example.com");
        assert!(body.get("password_hash").is_none());
    }

    #[tokio::test]
    async fn test_get_me_unauthenticated() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/users/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // PUT /api/v1/users/me
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_update_profile_display_name() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "bob", "bob@example.com").await;

        let payload = serde_json::json!({ "display_name": "Robert" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/users/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["display_name"], "Robert");
    }

    #[tokio::test]
    async fn test_update_email_conflict() {
        let (app, _tmp) = test_app().await;
        register(&app, "carol", "carol@example.com").await;
        let (token_d, _) = register(&app, "dan", "dan@example.com").await;

        let payload = serde_json::json!({ "email": "carol@example.com" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/users/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token_d}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    // -----------------------------------------------------------------------
    // PUT /api/v1/users/me/password
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_change_password_success() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "eve", "eve@example.com").await;

        let payload =
            serde_json::json!({ "current_password": "testpass", "new_password": "newpass123" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/users/me/password")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_change_password_wrong_current() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "frank", "frank@example.com").await;

        let payload =
            serde_json::json!({ "current_password": "wrongpass", "new_password": "newpass" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/users/me/password")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // GET /api/v1/users/me/organizations
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_my_organizations() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "grace", "grace@example.com").await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/users/me/organizations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        let orgs = body.as_array().unwrap();
        // Registration creates one personal org
        assert_eq!(orgs.len(), 1);
        assert!(orgs[0]["name"].as_str().unwrap().contains("grace"));
    }

    // -----------------------------------------------------------------------
    // PUT /users/me/active-organization
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_set_active_organization_success() {
        let (app, _tmp) = test_app().await;

        let (token, _) = register(&app, "henry", "henry@example.com").await;

        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me_body = body_json(me_resp).await;
        let org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        let payload = serde_json::json!({ "organization_id": org_id });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/users/me/active-organization")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert!(body.get("password_hash").is_none());
        assert_eq!(body["active_organization_id"].as_str().unwrap(), org_id);
    }

    #[tokio::test]
    async fn test_set_active_organization_not_member() {
        let (app, _tmp) = test_app().await;

        let (token, _) = register(&app, "ida", "ida@example.com").await;

        let fake_org_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({ "organization_id": fake_org_id });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/users/me/active-organization")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_set_active_organization_unauthenticated() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "organization_id": "any-id" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/users/me/active-organization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_set_active_org_cross_user_forbidden() {
        let (app, _tmp) = test_app().await;

        let (token_a, _) = register(&app, "jack", "jack@example.com").await;
        let (token_b, _) = register(&app, "kate", "kate@example.com").await;

        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me_body = body_json(me_resp).await;
        let kate_org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        let payload = serde_json::json!({ "organization_id": kate_org_id });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/users/me/active-organization")
                    .header(header::AUTHORIZATION, format!("Bearer {token_a}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
