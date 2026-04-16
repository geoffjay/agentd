//! Auth middleware extractor for protected routes.
//!
//! [`AuthUser`] implements [`axum::extract::FromRequestParts`] and can be used
//! as a function parameter in any Axum handler to require a valid session token.
//!
//! # Example
//!
//! ```rust,ignore
//! async fn protected(auth: AuthUser) -> impl IntoResponse {
//!     Json(json!({ "user_id": auth.user.id }))
//! }
//! ```

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{api::AppState, entity::user};

/// Authenticated user extracted from a valid `Authorization: Bearer <token>` header.
pub struct AuthUser {
    /// The authenticated user model.
    pub user: user::Model,
    /// The raw session token (not stored — passed through for logout etc.).
    pub token: String,
}

/// Extractor error returned when authentication fails.
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "missing authorization token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid or unknown token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "session expired"),
            AuthError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract "Authorization: Bearer <token>"
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token =
            auth_header.strip_prefix("Bearer ").ok_or(AuthError::InvalidToken)?.trim().to_string();

        if token.is_empty() {
            return Err(AuthError::InvalidToken);
        }

        // Look up the session
        let session = state
            .storage
            .sessions()
            .get_by_token_hash(&token)
            .await
            .map_err(|_| AuthError::InternalError)?
            .ok_or(AuthError::InvalidToken)?;

        // Check expiry
        let now = chrono::Utc::now().to_rfc3339();
        if session.expires_at <= now {
            return Err(AuthError::ExpiredToken);
        }

        // Fetch the associated user
        let user = state
            .storage
            .users()
            .get_by_id(&session.user_id)
            .await
            .map_err(|_| AuthError::InternalError)?
            .ok_or(AuthError::InvalidToken)?;

        Ok(AuthUser { user, token })
    }
}
