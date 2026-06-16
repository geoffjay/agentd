//! REST API router for the agentd-knowledge service.
//!
//! ## Routes
//!
//! | Method   | Path                                          | Description              |
//! |----------|-----------------------------------------------|--------------------------|
//! | `GET`    | `/health`                                     | Health check             |
//! | `GET`    | `/projects/:project_id/documents`             | List (paginated)         |
//! | `POST`   | `/projects/:project_id/documents`             | Create (201)             |
//! | `GET`    | `/projects/:project_id/documents/:doc_id`     | Get metadata             |
//! | `GET`    | `/projects/:project_id/documents/:doc_id/content` | Get with content    |
//! | `PUT`    | `/projects/:project_id/documents/:doc_id`     | Update (optimistic CC)   |
//! | `DELETE` | `/projects/:project_id/documents/:doc_id`     | Delete (204)             |
//! | `DELETE` | `/projects/:project_id/documents`             | Bulk delete (204)        |
//! | `GET`    | `/projects/:project_id/tree`                  | Virtual folder/file tree |
//! | `GET`    | `/projects/:project_id/doctor`                | Reconciliation report    |
//! | `POST`   | `/projects/:project_id/doctor`                | Reconcile and fix        |

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::limit::RequestBodyLimitLayer;

use agentd_common::error::ApiError;
use agentd_common::tenant::OptionalTenantId;
use agentd_common::types::clamp_limit;

use crate::{
    error::KnowledgeError,
    storage::KnowledgeStorage,
    types::{CreateDocumentRequest, DoctorReport, TreeNode, UpdateDocumentRequest},
};

/// Maximum request body size: 5 MiB.
const BODY_LIMIT_BYTES: usize = 5 * 1024 * 1024;

/// Normalize the `X-Tenant-ID` header into an organization scope.
///
/// The core gateway injects `X-Tenant-ID` (the caller's active organization).
/// When present, every storage operation is scoped to that organization; when
/// absent or empty (trusted/local access without the gateway), operations fall
/// back to project-only scoping.
fn org_scope(tenant: &OptionalTenantId) -> Option<&str> {
    tenant.0.as_deref().filter(|s| !s.is_empty())
}

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<KnowledgeStorage>,
}

/// Create the Axum router (no persistent state — health only).
#[allow(dead_code)]
pub fn create_router() -> Router {
    Router::new().route("/health", get(health_handler))
}

/// Create the Axum router with shared storage state.
pub fn create_router_with_state(storage: Arc<KnowledgeStorage>) -> Router {
    let state = ApiState { storage };

    Router::new()
        .route("/health", get(health_handler))
        // Collection routes
        .route(
            "/projects/{project_id}/documents",
            get(list_documents).post(create_document).delete(bulk_delete_documents),
        )
        // Instance routes
        .route(
            "/projects/{project_id}/documents/{doc_id}",
            get(get_document).put(update_document).delete(delete_document),
        )
        .route("/projects/{project_id}/documents/{doc_id}/content", get(get_document_content))
        // Virtual tree
        .route("/projects/{project_id}/tree", get(get_tree))
        // Doctor / reconciliation
        .route("/projects/{project_id}/doctor", get(get_doctor).post(post_doctor))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "agentd-knowledge" }))
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ListParams {
    limit: Option<usize>,
    offset: Option<usize>,
    prefix: Option<String>,
}

async fn list_documents(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    tenant: OptionalTenantId,
    Query(params): Query<ListParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = clamp_limit(params.limit) as u64;
    let offset = params.offset.unwrap_or(0) as u64;

    let mut page = state
        .storage
        .list_documents(&project_id, org_scope(&tenant), limit, offset)
        .await
        .map_err(knowledge_to_api)?;

    // Apply optional prefix filter client-side (the column is already indexed
    // by rel_path, so a small result set is expected).
    if let Some(prefix) = &params.prefix {
        page.items.retain(|d| d.rel_path.starts_with(prefix.as_str()));
    }

    Ok(Json(json!({
        "items": page.items,
        "total": page.total,
        "limit": page.limit,
        "offset": page.offset,
    })))
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

async fn create_document(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    tenant: OptionalTenantId,
    Json(req): Json<CreateDocumentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Reject non-markdown before even touching storage.
    if !req.rel_path.ends_with(".md") {
        return Err(ApiError::InvalidInput("rel_path must have a .md extension".to_string()));
    }

    let org = org_scope(&tenant).map(|s| s.to_string());
    let doc =
        state.storage.create_document(&project_id, req, org).await.map_err(knowledge_to_api)?;

    metrics::counter!("knowledge_documents_created_total").increment(1);
    Ok((StatusCode::CREATED, Json(doc)))
}

// ---------------------------------------------------------------------------
// Get metadata
// ---------------------------------------------------------------------------

async fn get_document(
    State(state): State<ApiState>,
    Path((project_id, doc_id)): Path<(String, String)>,
    tenant: OptionalTenantId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let doc = state
        .storage
        .get_document(&project_id, &doc_id, org_scope(&tenant))
        .await
        .map_err(knowledge_to_api)?;

    Ok(Json(serde_json::to_value(&doc).unwrap()))
}

// ---------------------------------------------------------------------------
// Get with content
// ---------------------------------------------------------------------------

async fn get_document_content(
    State(state): State<ApiState>,
    Path((project_id, doc_id)): Path<(String, String)>,
    tenant: OptionalTenantId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let doc_content = state
        .storage
        .get_document_content(&project_id, &doc_id, org_scope(&tenant))
        .await
        .map_err(|e| {
            // If the DB row exists but the file is missing, emit a metric.
            if let KnowledgeError::Other(ref inner) = e {
                if inner.to_string().contains("read failed") {
                    metrics::counter!("knowledge_missing_file_total").increment(1);
                }
            }
            knowledge_to_api(e)
        })?;

    Ok(Json(serde_json::to_value(&doc_content).unwrap()))
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

async fn update_document(
    State(state): State<ApiState>,
    Path((project_id, doc_id)): Path<(String, String)>,
    tenant: OptionalTenantId,
    Json(req): Json<UpdateDocumentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updated = state
        .storage
        .update_document(&project_id, &doc_id, org_scope(&tenant), req)
        .await
        .map_err(knowledge_to_api)?;

    Ok(Json(serde_json::to_value(&updated).unwrap()))
}

// ---------------------------------------------------------------------------
// Delete single
// ---------------------------------------------------------------------------

async fn delete_document(
    State(state): State<ApiState>,
    Path((project_id, doc_id)): Path<(String, String)>,
    tenant: OptionalTenantId,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .delete_document(&project_id, &doc_id, org_scope(&tenant))
        .await
        .map_err(knowledge_to_api)?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Bulk delete
// ---------------------------------------------------------------------------

async fn bulk_delete_documents(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    tenant: OptionalTenantId,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .bulk_delete_project(&project_id, org_scope(&tenant))
        .await
        .map_err(knowledge_to_api)?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Virtual tree
// ---------------------------------------------------------------------------

async fn get_tree(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    tenant: OptionalTenantId,
) -> Result<Json<Vec<TreeNode>>, ApiError> {
    // Fetch all docs — tree is built from the full set.
    let page = state
        .storage
        .list_documents(&project_id, org_scope(&tenant), 10_000, 0)
        .await
        .map_err(knowledge_to_api)?;

    let tree = build_tree(&page.items);
    Ok(Json(tree))
}

/// Build a virtual folder/file tree from a flat list of documents.
///
/// Each unique path prefix becomes a `Folder` node; each document
/// becomes a `File` leaf. Nodes at the same level are sorted by name.
fn build_tree(docs: &[crate::types::Document]) -> Vec<TreeNode> {
    // Use a recursive helper that builds a level given a path prefix.
    build_level(docs, "")
}

fn build_level(docs: &[crate::types::Document], prefix: &str) -> Vec<TreeNode> {
    // Collect immediate folder names and direct files under `prefix`.
    // Recursion uses the full `docs` slice — no need to pre-partition.
    let mut folder_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut files: Vec<TreeNode> = Vec::new();

    for doc in docs {
        // Strip the prefix and get the remainder.
        let rel = if prefix.is_empty() {
            doc.rel_path.as_str()
        } else {
            match doc.rel_path.strip_prefix(prefix) {
                Some(r) => r.trim_start_matches('/'),
                None => continue, // doesn't belong to this prefix
            }
        };

        if let Some(slash_pos) = rel.find('/') {
            // This doc belongs to a subfolder.
            folder_names.insert(rel[..slash_pos].to_string());
        } else {
            // This doc is a direct file under `prefix`.
            let name = std::path::Path::new(rel)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| rel.to_string());
            files.push(TreeNode::File { name, path: doc.rel_path.clone(), doc_id: doc.id.clone() });
        }
    }

    let mut nodes: Vec<TreeNode> = folder_names
        .into_iter()
        .map(|folder_name| {
            let child_prefix = if prefix.is_empty() {
                folder_name.clone()
            } else {
                format!("{prefix}/{folder_name}")
            };
            let children = build_level(docs, &child_prefix);
            TreeNode::Folder { name: folder_name, path: child_prefix, children }
        })
        .collect();

    nodes.append(&mut files);
    nodes
}

// ---------------------------------------------------------------------------
// Doctor / reconciliation
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DoctorQuery {
    /// When `true`, delete orphaned disk files and stale DB rows automatically.
    fix: Option<bool>,
}

/// `GET /projects/{project_id}/doctor` — report divergences, no changes.
async fn get_doctor(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
) -> Result<Json<DoctorReport>, ApiError> {
    let report = state.storage.doctor(&project_id, false).await.map_err(knowledge_to_api)?;
    Ok(Json(report))
}

/// `POST /projects/{project_id}/doctor[?fix=true]` — reconcile and optionally fix.
async fn post_doctor(
    State(state): State<ApiState>,
    Path(project_id): Path<String>,
    Query(params): Query<DoctorQuery>,
) -> Result<Json<DoctorReport>, ApiError> {
    let fix = params.fix.unwrap_or(true);
    let report = state.storage.doctor(&project_id, fix).await.map_err(knowledge_to_api)?;
    Ok(Json(report))
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

/// Map a `KnowledgeError` to an `ApiError` for HTTP responses.
pub fn knowledge_to_api(e: KnowledgeError) -> ApiError {
    match e {
        KnowledgeError::NotFound(msg) => {
            let _ = msg; // context already in ApiError::NotFound display
            ApiError::NotFound
        }
        KnowledgeError::Conflict(msg) => ApiError::Conflict(msg),
        KnowledgeError::InvalidPath(msg) => ApiError::InvalidInput(msg),
        KnowledgeError::Other(e) => ApiError::Internal(e),
    }
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tower::ServiceExt; // for `oneshot`

    const PROJ: &str = "550e8400-e29b-41d4-a716-446655440002";

    /// Create a shared storage and a factory that produces a fresh router per
    /// request (avoids `Router::clone` state-sharing issues with oneshot).
    async fn make_storage(tmp: &TempDir) -> Arc<KnowledgeStorage> {
        let db_path = tmp.path().join("test.db");
        let root = tmp.path().join("docs");
        Arc::new(KnowledgeStorage::with_path(&db_path, &root).await.expect("storage init"))
    }

    fn app(storage: Arc<KnowledgeStorage>) -> Router {
        create_router_with_state(storage)
    }

    async fn body_json(b: Body) -> serde_json::Value {
        let bytes = b.collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn json_req(method: Method, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    fn get_req(uri: &str) -> Request<Body> {
        Request::builder().method(Method::GET).uri(uri).body(Body::empty()).unwrap()
    }

    fn delete_req(uri: &str) -> Request<Body> {
        Request::builder().method(Method::DELETE).uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn test_health() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;
        let res = app(Arc::clone(&st)).oneshot(get_req("/health")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let json = body_json(res.into_body()).await;
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        // Create a document.
        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "readme.md", "content": "# Readme" }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let created = body_json(res.into_body()).await;
        assert_eq!(created["rel_path"], "readme.md");
        assert_eq!(created["title"], "readme");

        // List documents.
        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let list = body_json(res.into_body()).await;
        assert_eq!(list["total"], 1);
    }

    #[tokio::test]
    async fn test_tenant_scoping_via_header() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        // Create a document as org-a (gateway injects X-Tenant-ID).
        let mut req = json_req(
            Method::POST,
            &format!("/projects/{PROJ}/documents"),
            json!({ "rel_path": "secret.md", "content": "# Secret" }),
        );
        req.headers_mut().insert("X-Tenant-ID", "org-a".parse().unwrap());
        let res = app(Arc::clone(&st)).oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);

        // org-b must not see it.
        let mut req = get_req(&format!("/projects/{PROJ}/documents"));
        req.headers_mut().insert("X-Tenant-ID", "org-b".parse().unwrap());
        let res = app(Arc::clone(&st)).oneshot(req).await.unwrap();
        let list = body_json(res.into_body()).await;
        assert_eq!(list["total"], 0, "org-b leaked org-a's document");

        // org-a sees it.
        let mut req = get_req(&format!("/projects/{PROJ}/documents"));
        req.headers_mut().insert("X-Tenant-ID", "org-a".parse().unwrap());
        let res = app(Arc::clone(&st)).oneshot(req).await.unwrap();
        let list = body_json(res.into_body()).await;
        assert_eq!(list["total"], 1);

        // Unscoped (no header) trusted access sees it too.
        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents")))
            .await
            .unwrap();
        let list = body_json(res.into_body()).await;
        assert_eq!(list["total"], 1);
    }

    #[tokio::test]
    async fn test_get_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "meta.md", "content": "content" }),
            ))
            .await
            .unwrap();
        let created = body_json(res.into_body()).await;
        let doc_id = created["id"].as_str().unwrap().to_string();

        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents/{doc_id}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let meta = body_json(res.into_body()).await;
        assert_eq!(meta["rel_path"], "meta.md");
    }

    #[tokio::test]
    async fn test_get_content() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "content_test.md", "content": "# Hello World" }),
            ))
            .await
            .unwrap();
        let created = body_json(res.into_body()).await;
        let doc_id = created["id"].as_str().unwrap().to_string();

        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents/{doc_id}/content")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let with_content = body_json(res.into_body()).await;
        assert_eq!(with_content["content"], "# Hello World");
    }

    #[tokio::test]
    async fn test_update_document() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "update_me.md", "content": "v1" }),
            ))
            .await
            .unwrap();
        let created = body_json(res.into_body()).await;
        let doc_id = created["id"].as_str().unwrap().to_string();

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::PUT,
                &format!("/projects/{PROJ}/documents/{doc_id}"),
                json!({ "content": "v2", "title": "Updated" }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let updated = body_json(res.into_body()).await;
        assert_eq!(updated["title"], "Updated");
    }

    #[tokio::test]
    async fn test_delete_document() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "todelete.md", "content": "bye" }),
            ))
            .await
            .unwrap();
        let created = body_json(res.into_body()).await;
        let doc_id = created["id"].as_str().unwrap().to_string();

        let res = app(Arc::clone(&st))
            .oneshot(delete_req(&format!("/projects/{PROJ}/documents/{doc_id}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        // Now get should 404.
        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents/{doc_id}")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_bulk_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        for name in &["a.md", "b.md", "c.md"] {
            app(Arc::clone(&st))
                .oneshot(json_req(
                    Method::POST,
                    &format!("/projects/{PROJ}/documents"),
                    json!({ "rel_path": name, "content": "x" }),
                ))
                .await
                .unwrap();
        }

        let res = app(Arc::clone(&st))
            .oneshot(delete_req(&format!("/projects/{PROJ}/documents")))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let res = app(Arc::clone(&st))
            .oneshot(get_req(&format!("/projects/{PROJ}/documents")))
            .await
            .unwrap();
        let list = body_json(res.into_body()).await;
        assert_eq!(list["total"], 0);
    }

    #[tokio::test]
    async fn test_conflict_on_duplicate_path() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "dup.md", "content": "first" }),
            ))
            .await
            .unwrap();

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "dup.md", "content": "second" }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_non_md_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "notes.txt", "content": "x" }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_optimistic_concurrency_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::POST,
                &format!("/projects/{PROJ}/documents"),
                json!({ "rel_path": "oc.md", "content": "v1" }),
            ))
            .await
            .unwrap();
        let created = body_json(res.into_body()).await;
        let doc_id = created["id"].as_str().unwrap().to_string();

        // Stale updated_at should trigger conflict.
        let res = app(Arc::clone(&st))
            .oneshot(json_req(
                Method::PUT,
                &format!("/projects/{PROJ}/documents/{doc_id}"),
                json!({
                    "content": "v2",
                    "expected_updated_at": "1970-01-01T00:00:00+00:00"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_virtual_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let st = make_storage(&tmp).await;

        for path in &["readme.md", "docs/api.md", "docs/guide.md", "docs/deep/notes.md"] {
            app(Arc::clone(&st))
                .oneshot(json_req(
                    Method::POST,
                    &format!("/projects/{PROJ}/documents"),
                    json!({ "rel_path": path, "content": "x" }),
                ))
                .await
                .unwrap();
        }

        let res =
            app(Arc::clone(&st)).oneshot(get_req(&format!("/projects/{PROJ}/tree"))).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let tree = body_json(res.into_body()).await;
        // Root should have a file (readme) and a folder (docs).
        let nodes = tree.as_array().unwrap();
        assert!(!nodes.is_empty());
        // At least one folder named "docs".
        assert!(nodes.iter().any(|n| n["type"] == "folder" && n["name"] == "docs"));
    }
}
