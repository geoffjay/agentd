//! Typed error variants for the communicate service HTTP client.
//!
//! [`CommunicateError`] lets callers match on specific HTTP-level outcomes
//! (conflict, not-found) without parsing free-form error strings.

/// Typed error returned by [`crate::client::CommunicateClient`] operations.
///
/// Variants correspond to distinct HTTP-level outcomes so callers can match
/// on them rather than substring-scanning error messages.
#[derive(Debug, thiserror::Error)]
pub enum CommunicateError {
    /// The requested resource already exists or the operation would create a
    /// duplicate (HTTP 409 Conflict).
    ///
    /// Example: adding a participant who is already a member of a room.
    #[error("conflict")]
    Conflict,

    /// The requested resource does not exist (HTTP 404 Not Found).
    ///
    /// Example: removing a participant from a room they are not a member of.
    #[error("not found")]
    NotFound,

    /// Any other error (transport failure, unexpected HTTP status, etc.).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
