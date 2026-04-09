//! User profile endpoint handlers.
//!
//! # Endpoints
//!
//! | Method | Path                              | Auth | Description                          |
//! |--------|-----------------------------------|------|--------------------------------------|
//! | PUT    | `/users/me/active-organization`   | Yes  | Switch the user's active organization |
//!
//! All endpoints require a valid `Authorization: Bearer <token>` header via
//! the [`crate::middleware::auth::AuthUser`] extractor.

use axum::{extract::State, response::IntoResponse, routing::put, Json, Router};
use serde::Deserialize;

use agentd_common::error::ApiError;

use crate::middleware::auth::AuthUser;

use super::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SetActiveOrganizationRequest {
    pub organization_id: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new().route("/me/active-organization", put(set_active_organization_handler))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

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

    // Return the updated user without password_hash
    Ok(Json(crate::api::auth::UserResponse::from(updated)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::api::{auth::UserResponse, AppState};
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
        let state = AppState { storage };
        let app = crate::api::create_router(state);
        (app, tmp)
    }

    /// Register a user and return `(token, user_id)`.
    async fn register_and_login(app: &Router, username: &str, email: &str) -> (String, String) {
        let payload = serde_json::json!({
            "username": username,
            "email": email,
            "password": "testpass"
        });
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
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let token = body["token"].as_str().unwrap().to_string();
        let user_id = body["user"]["id"].as_str().unwrap().to_string();
        (token, user_id)
    }

    #[tokio::test]
    async fn test_set_active_organization_success() {
        let (app, _tmp) = test_app().await;

        // Register user (creates personal org and sets it as active)
        let (token, _user_id) = register_and_login(&app, "alice", "alice@example.com").await;

        // Get current active org
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
        let me_bytes = me_resp.into_body().collect().await.unwrap().to_bytes();
        let me_body: serde_json::Value = serde_json::from_slice(&me_bytes).unwrap();
        let org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        // Switch back to the same org (idempotent)
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
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // Response is UserResponse — no password_hash field
        assert!(body.get("password_hash").is_none());
        assert_eq!(body["active_organization_id"].as_str().unwrap(), org_id);
    }

    #[tokio::test]
    async fn test_set_active_organization_not_member() {
        let (app, _tmp) = test_app().await;

        let (token, _) = register_and_login(&app, "bob", "bob@example.com").await;

        // Use a random UUID that doesn't correspond to any org the user is in
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
    async fn test_set_active_org_allows_second_org() {
        let (app, _tmp) = test_app().await;

        // Register Alice (gets personal org automatically)
        let (token_a, user_a_id) = register_and_login(&app, "alice2", "alice2@example.com").await;
        let _ = user_a_id;

        // Register Bob (we'll use his personal org for the cross-org test)
        let (token_b, _) = register_and_login(&app, "bob2", "bob2@example.com").await;

        // Get Bob's personal org id
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
        let me_bytes = me_resp.into_body().collect().await.unwrap().to_bytes();
        let me_body: serde_json::Value = serde_json::from_slice(&me_bytes).unwrap();
        let bob_org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        // Alice tries to switch to Bob's org (she's not a member)
        let payload = serde_json::json!({ "organization_id": bob_org_id });
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

    // Keep the compiler happy with unused import
    #[allow(dead_code)]
    fn _use_user_response(_: UserResponse) {}
}
