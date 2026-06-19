//! Project management endpoint handlers.
//!
//! # Endpoints
//!
//! | Method | Path                   | Description                               |
//! |--------|------------------------|-------------------------------------------|
//! | POST   | `/api/v1/projects`     | Create a project (org from tenant header) |
//! | GET    | `/api/v1/projects`     | List projects, optionally scoped to org   |
//! | GET    | `/api/v1/projects/{id}`| Get project by UUID                       |
//! | PUT    | `/api/v1/projects/{id}`| Update project name and/or description    |
//! | DELETE | `/api/v1/projects/{id}`| Delete project                            |

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agentd_common::error::ApiError;
use agentd_common::tenant::OptionalTenantId;

use super::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectListQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub organization_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_project_handler))
        .route("/", get(list_projects_handler))
        .route("/{id}", get(get_project_handler))
        .route("/{id}", put(update_project_handler))
        .route("/{id}", delete(delete_project_handler))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/v1/projects`
///
/// Creates a new project. The `organization_id` is taken from the
/// `X-Tenant-ID` header when present (forwarded by the core gateway).
///
/// Returns `201 Created` with the project payload.
async fn create_project_handler(
    OptionalTenantId(org_id): OptionalTenantId,
    State(state): State<AppState>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.name.trim().is_empty() {
        return Err(ApiError::InvalidInput("project name must not be empty".to_string()));
    }

    let project = state
        .storage
        .projects()
        .create(&body.name, body.description.as_deref(), org_id.as_deref())
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                ApiError::Conflict(format!("a project named '{}' already exists", body.name))
            } else {
                ApiError::Internal(e)
            }
        })?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse {
            id: project.id,
            name: project.name,
            description: project.description,
            organization_id: project.organization_id,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }),
    ))
}

/// `GET /api/v1/projects`
///
/// Lists projects. When an `X-Tenant-ID` header is present (forwarded by
/// the core gateway), only projects belonging to that organization are
/// returned (plus legacy NULL-org rows). Supports `limit` and `offset`
/// query parameters for pagination.
async fn list_projects_handler(
    OptionalTenantId(org_id): OptionalTenantId,
    State(state): State<AppState>,
    Query(query): Query<ProjectListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let all =
        state.storage.projects().list_org(org_id.as_deref()).await.map_err(ApiError::Internal)?;

    let total = all.len();
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    let items: Vec<ProjectResponse> = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|p| ProjectResponse {
            id: p.id,
            name: p.name,
            description: p.description,
            organization_id: p.organization_id,
            created_at: p.created_at,
            updated_at: p.updated_at,
        })
        .collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

/// `GET /api/v1/projects/{id}`
///
/// Returns project details for the given UUID. Returns `404` if not found.
async fn get_project_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let project = state
        .storage
        .projects()
        .get_by_id(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(ProjectResponse {
        id: project.id,
        name: project.name,
        description: project.description,
        organization_id: project.organization_id,
        created_at: project.created_at,
        updated_at: project.updated_at,
    }))
}

/// `PUT /api/v1/projects/{id}`
///
/// Updates the project's `name` and/or `description`. Fields absent from
/// the body are left unchanged. Returns `404` if not found, `409` if the
/// new name conflicts with an existing project.
async fn update_project_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let project = state
        .storage
        .projects()
        .update(&id, body.name.as_deref(), body.description.as_deref())
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("not found") {
                ApiError::NotFound
            } else if msg.contains("UNIQUE") {
                ApiError::Conflict("a project with that name already exists".to_string())
            } else {
                ApiError::Internal(e)
            }
        })?;

    Ok(Json(ProjectResponse {
        id: project.id,
        name: project.name,
        description: project.description,
        organization_id: project.organization_id,
        created_at: project.created_at,
        updated_at: project.updated_at,
    }))
}

/// `DELETE /api/v1/projects/{id}`
///
/// Deletes the project. Core only checks its own constraints — there is no
/// cross-service delete-guard at this layer. Returns `204 No Content`.
async fn delete_project_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let deleted = state.storage.projects().delete(&id).await.map_err(ApiError::Internal)?;

    if !deleted {
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
        let state = AppState { storage };
        let app = crate::api::create_router(state);
        (app, tmp)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    // -----------------------------------------------------------------------
    // Create project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_project_returns_201() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Test Project" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["name"], "Test Project");
        assert!(body["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_create_project_with_description() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({
            "name": "Described Project",
            "description": "A helpful description"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["description"], "A helpful description");
    }

    #[tokio::test]
    async fn test_create_project_empty_name_returns_400() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "   " });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_duplicate_project_returns_409() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Duplicate Project" });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_create_project_sets_org_from_tenant_header() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Tenant Project" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("X-Tenant-ID", "org-abc")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        assert_eq!(body["organization_id"], "org-abc");
    }

    // -----------------------------------------------------------------------
    // Get project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_project_returns_200() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Fetchable Project" });
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(create_resp).await;
        let id = created["id"].as_str().unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/projects/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["name"], "Fetchable Project");
    }

    #[tokio::test]
    async fn test_get_missing_project_returns_404() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/projects/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // -----------------------------------------------------------------------
    // List projects
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_projects_returns_all() {
        let (app, _tmp) = test_app().await;

        for name in &["Alpha", "Beta", "Gamma"] {
            let payload = serde_json::json!({ "name": name });
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/projects")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["total"], 3);
        assert_eq!(body["items"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_list_projects_with_pagination() {
        let (app, _tmp) = test_app().await;

        for i in 0..5u32 {
            let payload = serde_json::json!({ "name": format!("Proj {i}") });
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/projects")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/projects?limit=2&offset=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["total"], 5);
        assert_eq!(body["items"].as_array().unwrap().len(), 2);
        assert_eq!(body["limit"], 2);
        assert_eq!(body["offset"], 1);
    }

    // -----------------------------------------------------------------------
    // Update project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_update_project_returns_200() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "Original Name" });
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(create_resp).await;
        let id = created["id"].as_str().unwrap();

        let update_payload = serde_json::json!({ "name": "Updated Name" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/projects/{id}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(update_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["name"], "Updated Name");
    }

    #[tokio::test]
    async fn test_update_missing_project_returns_404() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "New Name" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/projects/nonexistent-id")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // -----------------------------------------------------------------------
    // Delete project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_delete_project_returns_204() {
        let (app, _tmp) = test_app().await;

        let payload = serde_json::json!({ "name": "To Delete" });
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/projects")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created = body_json(create_resp).await;
        let id = created["id"].as_str().unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/projects/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_missing_project_returns_404() {
        let (app, _tmp) = test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/projects/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
