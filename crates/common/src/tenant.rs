//! Axum extractors for reading the `X-Tenant-ID` header injected by the core
//! gateway.
//!
//! The core gateway validates each incoming request, resolves the caller's
//! active organization, and injects `X-Tenant-ID` before forwarding to a
//! downstream service. Downstream services use these extractors to read that
//! header and scope their database queries accordingly.
//!
//! # Extractors
//!
//! - [`TenantId`] — requires the header; returns `400 Bad Request` if absent,
//!   unless `AGENTD_REQUIRE_TENANT=false` (the default), in which case it falls
//!   back to an empty string so services work without the gateway in dev/test.
//! - [`OptionalTenantId`] — always succeeds; yields `Option<String>` (None when
//!   the header is absent). Use this for endpoints that must serve both
//!   authenticated (gateway-routed) and local/trusted (MCP, tests) callers.
//!
//! # Configuration
//!
//! | Env var                  | Default | Effect                                          |
//! |--------------------------|---------|--------------------------------------------------|
//! | `AGENTD_REQUIRE_TENANT`  | `false` | When `true`, `TenantId` rejects missing headers  |
//!
//! # Example
//!
//! ```rust,ignore
//! use agentd_common::tenant::{TenantId, OptionalTenantId};
//!
//! // Strict: requires X-Tenant-ID (or permissive fallback in dev)
//! async fn scoped_handler(tenant: TenantId) -> impl IntoResponse {
//!     Json(json!({ "tenant": tenant.0 }))
//! }
//!
//! // Optional: works with or without the gateway
//! async fn flexible_handler(tenant: OptionalTenantId) -> impl IntoResponse {
//!     match tenant.0 {
//!         Some(id) => Json(json!({ "tenant": id })),
//!         None => Json(json!({ "tenant": null })),
//!     }
//! }
//! ```

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Header name
// ---------------------------------------------------------------------------

const X_TENANT_ID: &str = "x-tenant-id";

// ---------------------------------------------------------------------------
// TenantId
// ---------------------------------------------------------------------------

/// Extracts the `X-Tenant-ID` header value from an incoming request.
///
/// In **strict mode** (`AGENTD_REQUIRE_TENANT=true`) the extractor rejects
/// requests that are missing the header with `400 Bad Request`.
///
/// In **permissive mode** (the default, `AGENTD_REQUIRE_TENANT=false`) the
/// extractor succeeds with an empty string when the header is absent. This
/// lets services run unmodified in local dev without the gateway.
#[derive(Debug, Clone)]
pub struct TenantId(pub String);

/// Rejection returned by [`TenantId`] in strict mode.
pub struct MissingTenantId;

impl IntoResponse for MissingTenantId {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(json!({ "error": "missing required X-Tenant-ID header" })))
            .into_response()
    }
}

/// Returns `true` when `AGENTD_REQUIRE_TENANT=true`.
fn tenant_required() -> bool {
    std::env::var("AGENTD_REQUIRE_TENANT")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

impl<S> FromRequestParts<S> for TenantId
where
    S: Send + Sync,
{
    type Rejection = MissingTenantId;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value =
            parts.headers.get(X_TENANT_ID).and_then(|v| v.to_str().ok()).map(|s| s.to_string());

        match value {
            Some(id) => Ok(TenantId(id)),
            None if tenant_required() => Err(MissingTenantId),
            None => Ok(TenantId(String::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// OptionalTenantId
// ---------------------------------------------------------------------------

/// Extracts the `X-Tenant-ID` header value from an incoming request.
///
/// Unlike [`TenantId`], this extractor never rejects a request. When the
/// header is absent the inner value is `None`. Use this for endpoints that
/// must serve both gateway-routed (authenticated) and local/trusted callers
/// such as the MCP server or internal health checks.
#[derive(Debug, Clone)]
pub struct OptionalTenantId(pub Option<String>);

impl<S> FromRequestParts<S> for OptionalTenantId
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value =
            parts.headers.get(X_TENANT_ID).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        Ok(OptionalTenantId(value))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt as _;

    // Helper: build a minimal router that echoes the extracted tenant id.
    fn tenant_id_app() -> Router {
        Router::new().route("/", get(|t: TenantId| async move { Json(json!({ "tenant": t.0 })) }))
    }

    fn optional_tenant_id_app() -> Router {
        Router::new()
            .route("/", get(|t: OptionalTenantId| async move { Json(json!({ "tenant": t.0 })) }))
    }

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // -- TenantId (permissive mode) --

    #[tokio::test]
    async fn tenant_id_present() {
        std::env::remove_var("AGENTD_REQUIRE_TENANT");
        let app = tenant_id_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("X-Tenant-ID", "org-abc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(body["tenant"], "org-abc");
    }

    /// Combines permissive and strict env-var tests in a single sequential
    /// test to avoid cross-runtime races on `AGENTD_REQUIRE_TENANT`.
    #[tokio::test]
    async fn tenant_id_missing_env_modes() {
        // --- permissive (default) ---
        std::env::remove_var("AGENTD_REQUIRE_TENANT");
        let resp = tenant_id_app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "expected 200 in permissive mode");
        let body = body_json(resp).await;
        assert_eq!(body["tenant"], "", "expected empty tenant id in permissive mode");

        // --- strict ---
        std::env::set_var("AGENTD_REQUIRE_TENANT", "true");
        let resp = tenant_id_app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "expected 400 in strict mode");
        std::env::remove_var("AGENTD_REQUIRE_TENANT");
    }

    // -- OptionalTenantId --

    #[tokio::test]
    async fn optional_tenant_id_present() {
        let app = optional_tenant_id_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("X-Tenant-ID", "org-xyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(body["tenant"], "org-xyz");
    }

    #[tokio::test]
    async fn optional_tenant_id_absent() {
        let app = optional_tenant_id_app();
        let resp =
            app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert!(body["tenant"].is_null());
    }
}
