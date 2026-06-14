//! Authentication endpoint handlers.
//!
//! # Endpoints
//!
//! | Method | Path              | Auth required | Description                                     |
//! |--------|-------------------|---------------|-------------------------------------------------|
//! | POST   | `/auth/register`  | No            | Create account + default personal organization  |
//! | POST   | `/auth/login`     | No            | Verify credentials, issue session token         |
//! | POST   | `/auth/logout`    | Yes           | Delete current session                          |
//! | GET    | `/auth/me`        | Yes           | Return current user + active organization       |
//!
//! Session tokens are 256-bit random hex strings stored in the `sessions` table.
//! The default expiry is 24 hours, overridden by the `SESSION_EXPIRY_HOURS`
//! environment variable.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agentd_common::error::ApiError;

use crate::{
    entity::{organization, user},
    middleware::auth::AuthUser,
    session_storage::SessionStorage,
    user_storage::UserStorage,
};

use super::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    /// Optional human-readable display name (defaults to username).
    pub display_name: Option<String>,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Login by username OR email — exactly one must be provided.
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
}

/// Response for `POST /auth/validate`.
#[derive(Debug, Serialize)]
pub struct ValidateResponse {
    pub valid: bool,
    pub user_id: String,
    pub organization_id: Option<String>,
}

/// Public representation of a user — never includes `password_hash`.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: Option<String>,
    pub email: String,
    pub display_name: Option<String>,
    pub role: String,
    /// Product-level superuser flag — grants access to the `/admin` area.
    pub is_superuser: bool,
    pub active_organization_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<user::Model> for UserResponse {
    fn from(u: user::Model) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            display_name: u.display_name,
            role: u.role,
            is_superuser: u.is_superuser,
            active_organization_id: u.active_organization_id,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

/// Public representation of an organization.
#[derive(Debug, Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<organization::Model> for OrgResponse {
    fn from(o: organization::Model) -> Self {
        Self {
            id: o.id,
            name: o.name,
            slug: o.slug,
            created_at: o.created_at,
            updated_at: o.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
    pub active_organization: Option<OrgResponse>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: UserResponse,
    pub active_organization: Option<OrgResponse>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
        .route("/validate", post(validate_handler))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return session lifetime in hours from env, defaulting to 24.
fn session_expiry_hours() -> i64 {
    std::env::var("SESSION_EXPIRY_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(24i64)
}

/// Build a session `expires_at` RFC 3339 string.
fn session_expires_at() -> String {
    (chrono::Utc::now() + chrono::Duration::hours(session_expiry_hours())).to_rfc3339()
}

/// Slugify a string for use as an organization slug.
fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Fetch the organization by ID, returning `None` if the ID is None or not found.
async fn fetch_active_org(
    state: &AppState,
    org_id: Option<&str>,
) -> Result<Option<OrgResponse>, ApiError> {
    let Some(id) = org_id else { return Ok(None) };
    let org = state.storage.organizations().get_by_id(id).await.map_err(ApiError::Internal)?;
    Ok(org.map(OrgResponse::from))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /auth/register`
///
/// Creates a new user with a hashed password, a default personal organization,
/// and an owner membership. Sets the new org as the user's `active_organization`.
///
/// Returns `201 Created` with a `LoginResponse` (token + user + active_org).
async fn register_handler(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.password.is_empty() {
        return Err(ApiError::InvalidInput("password must not be empty".into()));
    }

    let users = state.storage.users();
    let orgs = state.storage.organizations();
    let memberships = state.storage.memberships();
    let sessions = state.storage.sessions();

    // Check uniqueness up-front for friendly errors (DB constraint would also
    // catch duplicates, but the message would be opaque).
    if users.get_by_email(&body.email).await.map_err(ApiError::Internal)?.is_some() {
        return Err(ApiError::Conflict(format!("email already registered: {}", body.email)));
    }
    if users.get_by_username(&body.username).await.map_err(ApiError::Internal)?.is_some() {
        return Err(ApiError::Conflict(format!("username already taken: {}", body.username)));
    }

    let display_name = body.display_name.as_deref().unwrap_or(&body.username);

    // Create the user
    let user = users
        .create(Some(&body.username), &body.email, Some(display_name), &body.password, "user")
        .await
        .map_err(ApiError::Internal)?;

    // Create a default personal organization: name = "<username>'s workspace"
    let org_name = format!("{}'s workspace", body.username);
    let org_slug = slugify(&format!("{}-workspace", body.username));

    let org = orgs.create(&org_name, &org_slug).await.map_err(ApiError::Internal)?;

    // Add the user as owner of their personal org
    memberships.add_member(&user.id, &org.id, "owner").await.map_err(ApiError::Internal)?;

    // Set the new org as the user's active organization
    let user =
        users.set_active_organization(&user.id, Some(&org.id)).await.map_err(ApiError::Internal)?;

    // Create a session token
    let token = SessionStorage::generate_token();
    let expires_at = session_expires_at();
    sessions.create(&user.id, &token, &expires_at).await.map_err(ApiError::Internal)?;

    let active_org = Some(OrgResponse::from(org));

    Ok((
        StatusCode::CREATED,
        Json(LoginResponse {
            token,
            user: UserResponse::from(user),
            active_organization: active_org,
        }),
    ))
}

/// `POST /auth/login`
///
/// Accepts `{ username, password }` or `{ email, password }`. Verifies the
/// password against the stored argon2 hash. Cleans up expired sessions for
/// the user, then issues a fresh session token.
///
/// Returns `200 OK` with `{ token, user, active_organization }`.
async fn login_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let users = state.storage.users();
    let sessions = state.storage.sessions();

    // Resolve user by username or email
    let user = match (body.username.as_deref(), body.email.as_deref()) {
        (Some(u), _) => users
            .get_by_username(u)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::Unauthorized("invalid credentials".into()))?,
        (_, Some(e)) => users
            .get_by_email(e)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::Unauthorized("invalid credentials".into()))?,
        _ => {
            return Err(ApiError::InvalidInput("one of `username` or `email` is required".into()));
        }
    };

    // Verify password
    let ok = UserStorage::verify_password(&body.password, &user.password_hash)
        .map_err(ApiError::Internal)?;
    if !ok {
        return Err(ApiError::Unauthorized("invalid credentials".into()));
    }

    // Clean up expired sessions for this user
    sessions.delete_expired_for_user(&user.id).await.map_err(ApiError::Internal)?;

    // Issue a new session token
    let token = SessionStorage::generate_token();
    let expires_at = session_expires_at();
    sessions.create(&user.id, &token, &expires_at).await.map_err(ApiError::Internal)?;

    let active_org = fetch_active_org(&state, user.active_organization_id.as_deref()).await?;

    Ok(Json(LoginResponse {
        token,
        user: UserResponse::from(user),
        active_organization: active_org,
    }))
}

/// `POST /auth/logout`
///
/// Requires `Authorization: Bearer <token>`. Deletes the current session.
///
/// Returns `204 No Content`.
async fn logout_handler(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    state.storage.sessions().delete_by_token_hash(&auth.token).await.map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /auth/validate`
///
/// Accepts `Authorization: Bearer <token>`. Validates the token and returns
/// the resolved `user_id` and `organization_id` without touching session
/// expiry. Designed for use by WebSocket endpoints that need a lightweight
/// token check at connection-upgrade time.
///
/// Returns `200 OK` with `{ valid: true, user_id, organization_id }` on
/// success, or `401 Unauthorized` if the token is missing, unknown, or expired.
async fn validate_handler(auth: AuthUser) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(ValidateResponse {
        valid: true,
        user_id: auth.user.id,
        organization_id: auth.user.active_organization_id,
    }))
}

/// `GET /auth/me`
///
/// Requires `Authorization: Bearer <token>`. Returns current user info and
/// the active organization (if set).
///
/// Returns `200 OK` with `{ user, active_organization }`.
async fn me_handler(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<impl IntoResponse, ApiError> {
    let active_org = fetch_active_org(&state, auth.user.active_organization_id.as_deref()).await?;

    Ok(Json(MeResponse { user: UserResponse::from(auth.user), active_organization: active_org }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentd_common::storage::create_test_connection;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn test_app() -> (Router, tempfile::TempDir) {
        let (conn, tmp) = create_test_connection().await;
        let storage = crate::storage::Storage::new(conn).await.unwrap();
        let state = AppState { storage };
        let app = crate::api::create_router(state);
        (app, tmp)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    // -----------------------------------------------------------------------
    // Register
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_register_success() {
        let (app, _tmp) = test_app().await;
        let payload = serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "secret123"
        });
        let response = app
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

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert!(body["token"].as_str().is_some());
        assert_eq!(body["user"]["username"], "alice");
        assert_eq!(body["user"]["email"], "alice@example.com");
        assert!(body["user"]["password_hash"].is_null());
        assert!(body["active_organization"]["name"].as_str().is_some());
        assert!(body["user"]["active_organization_id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_register_duplicate_email() {
        let (app, _tmp) = test_app().await;

        // First registration
        let payload = serde_json::json!({
            "username": "bob",
            "email": "bob@example.com",
            "password": "pass"
        });
        app.clone()
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

        // Second registration with same email, different username
        let payload2 = serde_json::json!({
            "username": "bob2",
            "email": "bob@example.com",
            "password": "pass"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload2.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_register_duplicate_username() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({
            "username": "carol",
            "email": "carol@example.com",
            "password": "pass"
        });
        app.clone()
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

        let payload2 = serde_json::json!({
            "username": "carol",
            "email": "carol2@example.com",
            "password": "pass"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload2.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_register_empty_password() {
        let (app, _tmp) = test_app().await;
        let payload = serde_json::json!({
            "username": "dan",
            "email": "dan@example.com",
            "password": ""
        });
        let response = app
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

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Login
    // -----------------------------------------------------------------------

    async fn register_user(
        app: &Router,
        username: &str,
        email: &str,
        password: &str,
    ) -> serde_json::Value {
        let payload =
            serde_json::json!({ "username": username, "email": email, "password": password });
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
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn test_login_by_username() {
        let (app, _tmp) = test_app().await;
        register_user(&app, "eve", "eve@example.com", "hunter2").await;

        let payload = serde_json::json!({ "username": "eve", "password": "hunter2" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert!(body["token"].as_str().is_some());
        assert_eq!(body["user"]["username"], "eve");
    }

    #[tokio::test]
    async fn test_login_by_email() {
        let (app, _tmp) = test_app().await;
        register_user(&app, "frank", "frank@example.com", "pass123").await;

        let payload = serde_json::json!({ "email": "frank@example.com", "password": "pass123" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert!(body["token"].as_str().is_some());
        assert_eq!(body["user"]["email"], "frank@example.com");
    }

    #[tokio::test]
    async fn test_login_wrong_password() {
        let (app, _tmp) = test_app().await;
        register_user(&app, "grace", "grace@example.com", "correct").await;

        let payload = serde_json::json!({ "username": "grace", "password": "wrong" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_unknown_user() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "username": "nobody", "password": "pass" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_missing_identifier() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "password": "pass" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Logout
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_logout_success() {
        let (app, _tmp) = test_app().await;
        let reg = register_user(&app, "henry", "henry@example.com", "pass").await;
        let token = reg["token"].as_str().unwrap().to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/logout")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_logout_without_token() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder().method("POST").uri("/auth/logout").body(Body::empty()).unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_logout_invalid_token() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/logout")
                    .header(header::AUTHORIZATION, "Bearer deadbeef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // Me
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_me_success() {
        let (app, _tmp) = test_app().await;
        let reg = register_user(&app, "ida", "ida@example.com", "pass").await;
        let token = reg["token"].as_str().unwrap().to_string();

        let response = app
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

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["user"]["username"], "ida");
        assert!(body["active_organization"].is_object());
    }

    #[tokio::test]
    async fn test_me_without_token() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(Request::builder().method("GET").uri("/auth/me").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_token_unusable_after_logout() {
        let (app, _tmp) = test_app().await;
        let reg = register_user(&app, "jack", "jack@example.com", "pass").await;
        let token = reg["token"].as_str().unwrap().to_string();

        // Logout
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/logout")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Token should now be rejected on /me
        let response = app
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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // Validate
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_success() {
        let (app, _tmp) = test_app().await;
        let reg = register_user(&app, "kate", "kate@example.com", "pass").await;
        let token = reg["token"].as_str().unwrap().to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/validate")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["valid"], true);
        assert!(body["user_id"].as_str().is_some());
        // registration creates a personal org and sets it as active
        assert!(body["organization_id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_validate_invalid_token() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/validate")
                    .header(header::AUTHORIZATION, "Bearer notarealtoken")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_validate_missing_token() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/validate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
