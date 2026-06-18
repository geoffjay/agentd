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
//! The real implementation links the system PAM library and is gated behind
//! `all(unix, feature = "pam")`, so it builds and runs on both Linux (Linux-PAM)
//! and macOS (OpenPAM). Every other configuration (CI without the feature,
//! Windows) compiles a [`UnavailableVerifier`] stub so the crate always builds
//! and a stray `auth_provider = 'pam'` row fails **closed** (500) rather than
//! ever mis-authenticating.

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
        #[cfg(not(all(unix, feature = "pam")))]
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

#[cfg(all(unix, feature = "pam"))]
mod imp {
    //! Real PAM verifier built on raw `pam-sys` bindings plus a hand-written,
    //! non-interactive conversation. Using raw bindings (rather than a
    //! higher-level wrapper) keeps this building on both Linux-PAM and macOS
    //! OpenPAM — the two disagree on the conversation message-array ABI, which
    //! we bridge in [`message_at`].

    use super::{PamError, PasswordVerifier};
    use std::ffi::{c_void, CString};
    use std::os::raw::{c_char, c_int};
    use std::ptr;

    use pam_sys::{
        acct_mgmt, authenticate, end, start, PamConversation, PamFlag, PamHandle, PamMessage,
        PamMessageStyle, PamResponse, PamReturnCode,
    };

    /// Credentials passed to the conversation callback through `data_ptr`.
    struct Credentials {
        username: CString,
        password: CString,
    }

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
            // NUL bytes can't appear in C strings; reject as a failed auth.
            let creds = Credentials {
                username: CString::new(username).map_err(|_| PamError::AuthFailed)?,
                password: CString::new(password).map_err(|_| PamError::AuthFailed)?,
            };

            let conv = PamConversation {
                conv: Some(converse),
                data_ptr: &creds as *const Credentials as *mut c_void,
            };

            let mut handle: *mut PamHandle = ptr::null_mut();
            let rc = start(&self.service, Some(username), &conv, &mut handle);
            if rc != PamReturnCode::SUCCESS || handle.is_null() {
                return Err(PamError::Unavailable(anyhow::anyhow!(
                    "pam_start failed for service `{}`: {:?}",
                    self.service,
                    rc
                )));
            }

            // SAFETY: `handle` is non-null per the check above and remains valid
            // until `end`. `creds` outlives the whole transaction.
            let handle_ref = unsafe { &mut *handle };

            let auth_rc = authenticate(handle_ref, PamFlag::NONE);
            let result = if auth_rc == PamReturnCode::SUCCESS {
                match acct_mgmt(handle_ref, PamFlag::NONE) {
                    PamReturnCode::SUCCESS => Ok(()),
                    other => Err(classify_acct(other)),
                }
            } else {
                Err(classify_auth(auth_rc))
            };

            end(handle_ref, auth_rc);
            drop(creds);
            result
        }
    }

    /// Index the i-th conversation message. Linux-PAM passes an array of message
    /// *pointers*; OpenPAM (macOS/BSD) passes a pointer to a *contiguous array*
    /// of messages. Dereferencing the wrong way is the classic PAM portability
    /// bug, so it is isolated here.
    #[cfg(target_os = "linux")]
    unsafe fn message_at(msg: *mut *mut PamMessage, i: isize) -> *const PamMessage {
        *msg.offset(i)
    }
    #[cfg(not(target_os = "linux"))]
    unsafe fn message_at(msg: *mut *mut PamMessage, i: isize) -> *const PamMessage {
        (*msg).offset(i)
    }

    /// Non-interactive conversation: answer echo-off prompts (password) with the
    /// preset password and echo-on prompts (login) with the username; ignore
    /// informational/error messages. The response array and each string are
    /// allocated with libc so PAM can `free()` them.
    extern "C" fn converse(
        num_msg: c_int,
        msg: *mut *mut PamMessage,
        out_resp: *mut *mut PamResponse,
        appdata: *mut c_void,
    ) -> c_int {
        if num_msg <= 0 || msg.is_null() || out_resp.is_null() || appdata.is_null() {
            return PamReturnCode::CONV_ERR as c_int;
        }
        // SAFETY: `appdata` is the `&Credentials` we passed to `pam_start`, alive
        // for the duration of the transaction.
        let creds = unsafe { &*(appdata as *const Credentials) };

        let n = num_msg as usize;
        // SAFETY: zero-initialised array of `n` responses; PAM frees it.
        let resp =
            unsafe { libc::calloc(n, std::mem::size_of::<PamResponse>()) as *mut PamResponse };
        if resp.is_null() {
            return PamReturnCode::BUF_ERR as c_int;
        }

        for i in 0..n {
            // SAFETY: `i < n == num_msg`; `message_at` reads within the array.
            let m = unsafe { message_at(msg, i as isize) };
            if m.is_null() {
                continue;
            }
            let style = unsafe { (*m).msg_style };
            let reply: Option<&CString> = if style == PamMessageStyle::PROMPT_ECHO_OFF as c_int {
                Some(&creds.password)
            } else if style == PamMessageStyle::PROMPT_ECHO_ON as c_int {
                Some(&creds.username)
            } else {
                None
            };
            if let Some(value) = reply {
                // SAFETY: strdup with libc so PAM owns and frees the copy.
                let dup = unsafe { libc::strdup(value.as_ptr() as *const c_char) };
                unsafe { (*resp.add(i)).resp = dup };
            }
        }

        // SAFETY: hand ownership of the response array to PAM.
        unsafe { *out_resp = resp };
        PamReturnCode::SUCCESS as c_int
    }

    /// Map an `authenticate()` failure: anything that isn't a clear operational
    /// fault is treated as a credential rejection.
    fn classify_auth(rc: PamReturnCode) -> PamError {
        use PamReturnCode::*;
        match rc {
            AUTH_ERR | USER_UNKNOWN | CRED_INSUFFICIENT | PERM_DENIED | MAXTRIES => {
                PamError::AuthFailed
            }
            other => PamError::Unavailable(anyhow::anyhow!("pam_authenticate failed: {other:?}")),
        }
    }

    /// Map an `acct_mgmt()` failure: account-state problems are credential-level
    /// rejections; everything else is operational.
    fn classify_acct(rc: PamReturnCode) -> PamError {
        use PamReturnCode::*;
        match rc {
            ACCT_EXPIRED | NEW_AUTHTOK_REQD | AUTHTOK_EXPIRED | USER_UNKNOWN | PERM_DENIED => {
                PamError::AccountInvalid
            }
            other => PamError::Unavailable(anyhow::anyhow!("pam_acct_mgmt failed: {other:?}")),
        }
    }
}

/// Build the real PAM verifier for the given service.
#[cfg(all(unix, feature = "pam"))]
pub fn build_verifier(service: &str) -> Arc<dyn PasswordVerifier> {
    Arc::new(imp::PamVerifier::new(service))
}

/// Non-PAM builds: always fail closed.
#[cfg(not(all(unix, feature = "pam")))]
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
