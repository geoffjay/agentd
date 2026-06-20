//! Organization management endpoint handlers.
//!
//! # Endpoints
//!
//! | Method | Path                                    | Auth | Role   | Description                      |
//! |--------|-----------------------------------------|------|--------|----------------------------------|
//! | POST   | `/api/v1/organizations`                 | Yes  | any    | Create org (creator = owner)     |
//! | GET    | `/api/v1/organizations/:id`             | Yes  | member | Get org details                  |
//! | PUT    | `/api/v1/organizations/:id`             | Yes  | owner  | Update org name                  |
//! | DELETE | `/api/v1/organizations/:id`             | Yes  | owner  | Delete org                       |
//! | GET    | `/api/v1/organizations/:id/members`     | Yes  | member | List members with roles          |
//! | POST   | `/api/v1/organizations/:id/members`     | Yes  | owner  | Add member                       |
//! | DELETE | `/api/v1/organizations/:id/members/:uid`| Yes  | owner  | Remove member                    |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agentd_common::error::ApiError;

use crate::middleware::auth::AuthUser;

use super::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrganizationRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub id: String,
    pub user_id: String,
    pub organization_id: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_org_handler))
        .route("/{id}", get(get_org_handler))
        .route("/{id}", put(update_org_handler))
        .route("/{id}", delete(delete_org_handler))
        .route("/{id}/members", get(list_members_handler))
        .route("/{id}/members", post(add_member_handler))
        .route("/{id}/members/{user_id}", delete(remove_member_handler))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Require that the caller is a member of the given org.
///
/// Returns `403 Forbidden` if not a member.
async fn require_member(
    state: &AppState,
    user_id: &str,
    org_id: &str,
) -> Result<crate::entity::membership::Model, ApiError> {
    state
        .storage
        .memberships()
        .get_membership(user_id, org_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| {
            ApiError::Forbidden(format!("user is not a member of organization {org_id}"))
        })
}

/// Require that the caller is an owner of the given org.
///
/// Returns `403 Forbidden` if not an owner.
async fn require_owner(state: &AppState, user_id: &str, org_id: &str) -> Result<(), ApiError> {
    let mem = require_member(state, user_id, org_id).await?;
    if mem.role != "owner" {
        return Err(ApiError::Forbidden(format!("only owners may modify organization {org_id}")));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/v1/organizations`
///
/// Creates a new organization. The calling user becomes the owner and their
/// active organization is switched to the new org.
///
/// Returns `201 Created` with the organization payload.
async fn create_org_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateOrganizationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let org = state
        .storage
        .organizations()
        .create(&body.name, &body.slug)
        .await
        .map_err(ApiError::Internal)?;

    state
        .storage
        .memberships()
        .add_member(&auth.user.id, &org.id, "owner")
        .await
        .map_err(ApiError::Internal)?;

    Ok((
        StatusCode::CREATED,
        Json(OrganizationResponse {
            id: org.id,
            name: org.name,
            slug: org.slug,
            created_at: org.created_at,
            updated_at: org.updated_at,
        }),
    ))
}

/// `GET /api/v1/organizations/:id`
///
/// Returns organization details. Requires caller to be a member.
async fn get_org_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    require_member(&state, &auth.user.id, &id).await?;

    let org = state
        .storage
        .organizations()
        .get_by_id(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(OrganizationResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        created_at: org.created_at,
        updated_at: org.updated_at,
    }))
}

/// `PUT /api/v1/organizations/:id`
///
/// Updates organization name and/or slug. Requires owner role.
async fn update_org_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateOrganizationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    require_owner(&state, &auth.user.id, &id).await?;

    let org = state
        .storage
        .organizations()
        .update(&id, body.name.as_deref(), body.slug.as_deref())
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(OrganizationResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        created_at: org.created_at,
        updated_at: org.updated_at,
    }))
}

/// `DELETE /api/v1/organizations/:id`
///
/// Deletes the organization. Requires owner role. Clears `active_organization_id`
/// for all users who had this as their active org.
///
/// Returns `204 No Content`.
async fn delete_org_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    require_owner(&state, &auth.user.id, &id).await?;

    // Clear active_organization_id for affected users before deleting
    state
        .storage
        .users()
        .clear_active_organization_for_org(&id)
        .await
        .map_err(ApiError::Internal)?;

    let deleted = state.storage.organizations().delete(&id).await.map_err(ApiError::Internal)?;

    if !deleted {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/organizations/:id/members`
///
/// Lists all members of the organization. Requires caller to be a member.
async fn list_members_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    require_member(&state, &auth.user.id, &id).await?;

    let members =
        state.storage.memberships().list_members(&id).await.map_err(ApiError::Internal)?;

    let response: Vec<MemberResponse> = members
        .into_iter()
        .map(|m| MemberResponse {
            id: m.id,
            user_id: m.user_id,
            organization_id: m.organization_id,
            role: m.role,
            created_at: m.created_at,
            updated_at: m.updated_at,
        })
        .collect();

    Ok(Json(response))
}

/// `POST /api/v1/organizations/:id/members`
///
/// Adds a user to the organization. Requires owner role.
///
/// Returns `201 Created` with the membership record.
async fn add_member_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, ApiError> {
    require_owner(&state, &auth.user.id, &id).await?;

    // Verify the target user exists
    state
        .storage
        .users()
        .get_by_id(&body.user_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let mem = state
        .storage
        .memberships()
        .add_member(&body.user_id, &id, &body.role)
        .await
        .map_err(ApiError::Internal)?;

    Ok((
        StatusCode::CREATED,
        Json(MemberResponse {
            id: mem.id,
            user_id: mem.user_id,
            organization_id: mem.organization_id,
            role: mem.role,
            created_at: mem.created_at,
            updated_at: mem.updated_at,
        }),
    ))
}

/// `DELETE /api/v1/organizations/:id/members/:user_id`
///
/// Removes a user from the organization. Requires owner role. Returns `403` if
/// the caller tries to remove themselves as the last owner.
///
/// Returns `204 No Content`.
async fn remove_member_handler(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, target_user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    require_owner(&state, &auth.user.id, &id).await?;

    // Prevent removing the last owner
    if target_user_id == auth.user.id {
        let all_members =
            state.storage.memberships().list_members(&id).await.map_err(ApiError::Internal)?;
        let owner_count = all_members.iter().filter(|m| m.role == "owner").count();
        if owner_count <= 1 {
            return Err(ApiError::Forbidden(
                "cannot remove the last owner from an organization".into(),
            ));
        }
    }

    let removed = state
        .storage
        .memberships()
        .remove_member(&target_user_id, &id)
        .await
        .map_err(ApiError::Internal)?;

    if !removed {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
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
    // Create org
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_org_success() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "alice", "alice@example.com").await;

        let payload = serde_json::json!({ "name": "Test Corp", "slug": "test-corp" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/organizations")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["name"], "Test Corp");
        assert_eq!(body["slug"], "test-corp");
    }

    #[tokio::test]
    async fn test_create_org_unauthenticated() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Test", "slug": "test" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/organizations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // -----------------------------------------------------------------------
    // Get org
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_org_as_member() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "bob", "bob@example.com").await;

        // Get the personal org created during registration
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

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/organizations/{org_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["id"], org_id.as_str());
    }

    #[tokio::test]
    async fn test_get_org_not_member() {
        let (app, _tmp) = test_app().await;
        let (token_a, _) = register(&app, "carol", "carol@example.com").await;
        let (token_b, _) = register(&app, "dan", "dan@example.com").await;

        // Get carol's personal org
        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me_body = body_json(me_resp).await;
        let carol_org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        // Dan tries to get Carol's org
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/organizations/{carol_org_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    // -----------------------------------------------------------------------
    // Update org
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_update_org_as_owner() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "eve", "eve@example.com").await;

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

        let payload = serde_json::json!({ "name": "Updated Name" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/organizations/{org_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["name"], "Updated Name");
    }

    // -----------------------------------------------------------------------
    // Delete org
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_delete_org_as_owner() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "frank", "frank@example.com").await;

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

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/organizations/{org_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_org_not_owner() {
        let (app, _tmp) = test_app().await;
        let (token_a, _) = register(&app, "grace", "grace@example.com").await;
        let (token_b, _) = register(&app, "henry", "henry@example.com").await;

        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me_body = body_json(me_resp).await;
        let grace_org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        // First get henry's user_id via /auth/me
        let henry_me = app
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
        let henry_body = body_json(henry_me).await;
        let henry_id = henry_body["user"]["id"].as_str().unwrap().to_string();

        // Grace adds henry as member
        let add_payload = serde_json::json!({ "user_id": henry_id, "role": "member" });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/organizations/{grace_org_id}/members"))
                    .header(header::AUTHORIZATION, format!("Bearer {token_a}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(add_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Henry tries to delete grace's org — should be forbidden
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/organizations/{grace_org_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    // -----------------------------------------------------------------------
    // Members
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_members() {
        let (app, _tmp) = test_app().await;
        let (token, _) = register(&app, "ida", "ida@example.com").await;

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

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/organizations/{org_id}/members"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        let members = body.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["role"], "owner");
    }

    #[tokio::test]
    async fn test_add_member() {
        let (app, _tmp) = test_app().await;
        let (token_owner, _) = register(&app, "jack", "jack@example.com").await;
        let (_, user_b_id) = register(&app, "kate", "kate@example.com").await;

        let me_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header(header::AUTHORIZATION, format!("Bearer {token_owner}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let me_body = body_json(me_resp).await;
        let org_id = me_body["active_organization"]["id"].as_str().unwrap().to_string();

        let payload = serde_json::json!({ "user_id": user_b_id, "role": "member" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/organizations/{org_id}/members"))
                    .header(header::AUTHORIZATION, format!("Bearer {token_owner}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["user_id"], user_b_id.as_str());
        assert_eq!(body["role"], "member");
    }

    #[tokio::test]
    async fn test_remove_last_owner_blocked() {
        let (app, _tmp) = test_app().await;
        let (token, user_id) = register(&app, "liam", "liam@example.com").await;

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

        // Liam tries to remove himself as the only owner
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/organizations/{org_id}/members/{user_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
