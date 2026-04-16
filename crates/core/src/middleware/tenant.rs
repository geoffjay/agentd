//! Tenant context extractor for multi-tenant request isolation.
//!
//! [`TenantContext`] implements [`axum::extract::FromRequestParts`] and resolves
//! the authenticated user's active organization for every request. It is more
//! strict than [`super::auth::AuthUser`]: a valid session is required **and**
//! the user must have an active organization set. Requests without an active
//! org receive `403 Forbidden`.
//!
//! # Usage
//!
//! ```rust,ignore
//! async fn my_handler(ctx: TenantContext) -> impl IntoResponse {
//!     Json(json!({ "org": ctx.organization_id, "user": ctx.user_id }))
//! }
//! ```
//!
//! # X-Tenant-ID header injection
//!
//! The `organization_id` field is intended to be forwarded as an
//! `X-Tenant-ID` header when proxying requests to downstream services.
//! Downstream services can then use this header to scope their queries
//! without implementing their own auth stack.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::api::AppState;

/// Per-request tenant context resolved from the active session + organization.
pub struct TenantContext {
    /// The authenticated user's ID.
    pub user_id: String,
    /// The organization the user is currently acting as.
    pub organization_id: String,
    /// Raw bearer token — available for forwarding or invalidation.
    pub session_token: String,
}

/// Extractor rejection for failed tenant resolution.
pub enum TenantError {
    /// No `Authorization` header or unparseable value.
    MissingToken,
    /// Token not found in sessions table or session is expired.
    InvalidSession,
    /// Valid session but the user has no `active_organization_id` set.
    NoActiveOrganization,
    /// Internal storage error.
    InternalError,
}

impl IntoResponse for TenantError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            TenantError::MissingToken => (StatusCode::UNAUTHORIZED, "missing authorization token"),
            TenantError::InvalidSession => (StatusCode::UNAUTHORIZED, "invalid or expired session"),
            TenantError::NoActiveOrganization => {
                (StatusCode::FORBIDDEN, "no active organization set for this user")
            }
            TenantError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl FromRequestParts<AppState> for TenantContext {
    type Rejection = TenantError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract bearer token
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or(TenantError::MissingToken)?;

        // Look up session and check expiry
        let session = state
            .storage
            .sessions()
            .get_by_token_hash(&token)
            .await
            .map_err(|_| TenantError::InternalError)?
            .ok_or(TenantError::InvalidSession)?;

        let now = chrono::Utc::now().to_rfc3339();
        if session.expires_at <= now {
            return Err(TenantError::InvalidSession);
        }

        // Fetch the user and require an active organization
        let user = state
            .storage
            .users()
            .get_by_id(&session.user_id)
            .await
            .map_err(|_| TenantError::InternalError)?
            .ok_or(TenantError::InvalidSession)?;

        let organization_id =
            user.active_organization_id.ok_or(TenantError::NoActiveOrganization)?;

        Ok(TenantContext { user_id: user.id, organization_id, session_token: token })
    }
}
