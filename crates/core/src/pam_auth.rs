//! PAM (Pluggable Authentication Modules) password verification for
//! system-user logins.
//!
//! This is the `'pam'` half of the pluggable auth backend: instead of verifying
//! a password against an argon2 hash in the database, a user with
//! `auth_provider = 'pam'` is authenticated against the host's PAM stack for the
//! configured service (default `agentd` → `/etc/pam.d/agentd`). The argon2
//! `'local'` provider remains the default and the bootstrap/superuser escape
//! hatch.
//!
//! # Privilege requirement
//!
//! `pam_unix`'s SUID helper `unix_chkpwd` refuses to verify any account other
//! than the caller's own UID. So to authenticate *other* system users the
//! `agentd-core` process must be able to read `/etc/shadow` directly — i.e. run
//! as a **system** service whose account is in the `shadow` group (or use an
//! SSSD-backed stack, where `pam_sss` consults a privileged socket and needs no
//! local shadow access). See the deployment guide under `docs/`.
//!
//! # Build gating
//!
//! The real implementation links system `libpam` and is gated behind
//! `all(target_os = "linux", feature = "pam")` (the `pam-client2` FFI targets
//! Linux-PAM and does not build against macOS PAM). Every other configuration
//! (macOS dev, CI without the feature, Windows) compiles a
//! [`UnavailableVerifier`] stub so the crate always builds and a stray
//! `auth_provider = 'pam'` row fails **closed** (500) rather than ever
//! mis-authenticating.

use std::sync::Arc;

/// PAM settings derived from environment variables.
#[derive(Debug, Clone)]
pub struct PamConfig {
    /// Master switch (`AGENTD_PAM_ENABLED`). When `false`, `'pam'` users fail
    /// closed and just-in-time provisioning never triggers.
    pub enabled: bool,
    /// PAM service name (`AGENTD_PAM_SERVICE`, default `agentd`) →
    /// `/etc/pam.d/<service>`.
    pub service: String,
    /// Domain used to synthesize the (required, unique) email address of a
    /// just-in-time provisioned PAM user (`AGENTD_PAM_EMAIL_DOMAIN`, default
    /// `pam.local`): `<system-user>@<domain>`.
    pub email_domain: String,
}

impl PamConfig {
    /// Default configuration with PAM disabled.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            service: "agentd".to_string(),
            email_domain: "pam.local".to_string(),
        }
    }

    /// Load configuration from `AGENTD_PAM_*` environment variables.
    pub fn from_env() -> Self {
        let enabled = std::env::var("AGENTD_PAM_ENABLED")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let service = std::env::var("AGENTD_PAM_SERVICE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "agentd".to_string());
        let email_domain = std::env::var("AGENTD_PAM_EMAIL_DOMAIN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "pam.local".to_string());

        // Loudly warn if PAM is requested but this binary cannot honor it.
        #[cfg(not(all(target_os = "linux", feature = "pam")))]
        if enabled {
            tracing::warn!(
                "AGENTD_PAM_ENABLED is set but this build lacks the `pam` feature; \
                 PAM logins will be rejected. Rebuild agentd-core with `--features pam`."
            );
        }

        Self { enabled, service, email_domain }
    }

    /// Build the verifier matching this configuration. On a non-PAM build this
    /// is always [`UnavailableVerifier`].
    pub fn build_verifier(&self) -> Arc<dyn PasswordVerifier> {
        build_verifier(&self.service)
    }
}

/// Outcome of a failed PAM verification.
#[derive(Debug)]
pub enum PamError {
    /// Credentials rejected — wrong password or unknown user. Maps to 401.
    AuthFailed,
    /// Account exists but may not log in (expired, locked, password change
    /// required, ...). Surfaced by `acct_mgmt`. Maps to 401.
    AccountInvalid,
    /// The PAM stack could not be consulted (misconfiguration, missing helper,
    /// unreachable SSSD socket, feature not compiled in). Maps to 500.
    Unavailable(anyhow::Error),
}

/// Verifies a system user's password against the host authentication stack.
///
/// Implementations must be cheap to share via [`Arc`] and safe to call from a
/// blocking thread (the HTTP handler invokes them inside
/// `tokio::task::spawn_blocking`, since the PAM FFI is blocking).
pub trait PasswordVerifier: Send + Sync {
    /// Returns `Ok(())` when `password` authenticates `username`.
    fn verify(&self, username: &str, password: &str) -> Result<(), PamError>;
}

/// Fail-closed verifier used when PAM support is not compiled in (non-`pam`
/// build or non-unix target). Always reports the stack as unavailable so a
/// `'pam'` user can never be authenticated by accident.
pub struct UnavailableVerifier;

impl PasswordVerifier for UnavailableVerifier {
    fn verify(&self, _username: &str, _password: &str) -> Result<(), PamError> {
        Err(PamError::Unavailable(anyhow::anyhow!(
            "PAM support is not compiled into this build (rebuild with `--features pam`)"
        )))
    }
}

#[cfg(all(target_os = "linux", feature = "pam"))]
mod imp {
    use super::{PamError, PasswordVerifier};
    use pam_client2::conv_mock::Conversation;
    use pam_client2::{Context, ErrorCode, Flag};

    /// Real libpam-backed verifier. Runs the standard
    /// `authenticate()` + `acct_mgmt()` sequence with a non-interactive
    /// conversation that replays the supplied credentials.
    pub struct PamVerifier {
        service: String,
    }

    impl PamVerifier {
        pub fn new(service: impl Into<String>) -> Self {
            Self { service: service.into() }
        }
    }

    impl PasswordVerifier for PamVerifier {
        fn verify(&self, username: &str, password: &str) -> Result<(), PamError> {
            let mut ctx = Context::new(
                &self.service,
                Some(username),
                Conversation::with_credentials(username, password),
            )
            .map_err(|e| PamError::Unavailable(anyhow::anyhow!("PAM context init failed: {e}")))?;

            // Phase 1: verify the credentials.
            ctx.authenticate(Flag::NONE).map_err(|e| classify_auth(e.code()))?;
            // Phase 2: account validity (expired / locked / must-change-password).
            ctx.acct_mgmt(Flag::NONE).map_err(|e| classify_acct(e.code()))?;
            Ok(())
        }
    }

    /// Map an `authenticate()` failure: anything that isn't a clear operational
    /// fault is treated as a credential rejection.
    fn classify_auth(code: ErrorCode) -> PamError {
        match code {
            ErrorCode::AUTH_ERR
            | ErrorCode::USER_UNKNOWN
            | ErrorCode::CRED_INSUFFICIENT
            | ErrorCode::PERM_DENIED
            | ErrorCode::MAXTRIES => PamError::AuthFailed,
            other => PamError::Unavailable(anyhow::anyhow!("PAM authenticate failed: {other:?}")),
        }
    }

    /// Map an `acct_mgmt()` failure: account-state problems are credential-level
    /// rejections; everything else is operational.
    fn classify_acct(code: ErrorCode) -> PamError {
        match code {
            ErrorCode::ACCT_EXPIRED
            | ErrorCode::NEW_AUTHTOK_REQD
            | ErrorCode::AUTHTOK_EXPIRED
            | ErrorCode::USER_UNKNOWN
            | ErrorCode::PERM_DENIED => PamError::AccountInvalid,
            other => PamError::Unavailable(anyhow::anyhow!("PAM acct_mgmt failed: {other:?}")),
        }
    }
}

/// Build the real PAM verifier for the given service.
#[cfg(all(target_os = "linux", feature = "pam"))]
pub fn build_verifier(service: &str) -> Arc<dyn PasswordVerifier> {
    Arc::new(imp::PamVerifier::new(service))
}

/// Non-PAM builds: always fail closed.
#[cfg(not(all(target_os = "linux", feature = "pam")))]
pub fn build_verifier(_service: &str) -> Arc<dyn PasswordVerifier> {
    Arc::new(UnavailableVerifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_verifier_fails_closed() {
        let v = UnavailableVerifier;
        assert!(matches!(v.verify("root", "anything"), Err(PamError::Unavailable(_))));
    }

    #[test]
    fn disabled_config_defaults() {
        let cfg = PamConfig::disabled();
        assert!(!cfg.enabled);
        assert_eq!(cfg.service, "agentd");
        assert_eq!(cfg.email_domain, "pam.local");
    }
}
