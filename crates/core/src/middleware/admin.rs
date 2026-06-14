//! Product-admin authorization extractor.
//!
//! [`SuperUser`] builds on [`AuthUser`](crate::middleware::auth::AuthUser): it
//! validates the bearer token the same way, then additionally requires the
//! authenticated user to have the product-level `is_superuser` flag set. Any
//! handler taking `SuperUser` is therefore gated to superusers and returns
//! `403 Forbidden` otherwise — this is the real, backend-enforced access control
//! for the `/admin` area (the UI gate is cosmetic).

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{
    api::AppState,
    entity::user,
    middleware::auth::{AuthError, AuthUser},
};

/// Authenticated user that has been verified to be a product-level superuser.
pub struct SuperUser {
    /// The authenticated superuser model.
    pub user: user::Model,
    /// The raw session token.
    pub token: String,
}

/// Rejection for the [`SuperUser`] extractor.
pub enum SuperUserError {
    /// Authentication failed (missing/invalid/expired token).
    Auth(AuthError),
    /// Authenticated, but the user is not a superuser.
    Forbidden,
}

impl IntoResponse for SuperUserError {
    fn into_response(self) -> Response {
        match self {
            SuperUserError::Auth(e) => e.into_response(),
            SuperUserError::Forbidden => {
                (StatusCode::FORBIDDEN, Json(json!({ "error": "superuser access required" })))
                    .into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for SuperUser {
    type Rejection = SuperUserError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth =
            AuthUser::from_request_parts(parts, state).await.map_err(SuperUserError::Auth)?;
        if !auth.user.is_superuser {
            return Err(SuperUserError::Forbidden);
        }
        Ok(SuperUser { user: auth.user, token: auth.token })
    }
}
