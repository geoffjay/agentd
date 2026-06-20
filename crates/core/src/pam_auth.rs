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

/// PAM settings, loaded from the `[services.core.pam]` config section with any
/// `AGENTD_PAM_*` environment variables overlaid on top (env wins).
#[derive(Debug, Clone)]
pub struct PamConfig {
    /// Master switch (`AGENTD_PAM_ENABLED` / `pam.enabled`). When `false`,
    /// `'pam'` users fail closed and just-in-time provisioning never triggers.
    pub enabled: bool,
    /// PAM service name (`AGENTD_PAM_SERVICE` / `pam.service`, default `agentd`)
    /// → `/etc/pam.d/<service>`.
    pub service: String,
    /// Domain used to synthesize the (required, unique) email address of a
    /// just-in-time provisioned PAM user (`AGENTD_PAM_EMAIL_DOMAIN` /
    /// `pam.email_domain`, default `pam.local`): `<system-user>@<domain>`.
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

    /// Load PAM settings from the shared `[services.core.pam]` config section,
    /// then overlay any `AGENTD_PAM_*` environment variables (env wins).
    pub fn load() -> Self {
        let file = agentd_common::config::load().map(|c| c.services.core.pam).unwrap_or_default();
        Self::from_file_and_env(file)
    }

    /// Overlay `AGENTD_PAM_*` environment variables onto the file-derived
    /// settings. An env var that is unset (or, for the string fields, blank)
    /// leaves the corresponding config value untouched.
    fn from_file_and_env(file: agentd_common::config::CorePamConfig) -> Self {
        let enabled = std::env::var("AGENTD_PAM_ENABLED")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(file.enabled);
        let service = std::env::var("AGENTD_PAM_SERVICE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(file.service);
        let email_domain = std::env::var("AGENTD_PAM_EMAIL_DOMAIN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(file.email_domain);

        // Loudly warn if PAM is requested but this binary cannot honor it.
        #[cfg(not(all(unix, feature = "pam")))]
        if enabled {
            tracing::warn!(
                "PAM is enabled (AGENTD_PAM_ENABLED / [services.core.pam] enabled) but this \
                 build lacks the `pam` feature; PAM logins will be rejected. Rebuild \
                 agentd-core with `--features pam`."
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
    //! OpenPAM. The one true ABI disagreement between them is the *status-code
    //! numbering*, which `pam-sys` hard-codes for Linux; we re-interpret the raw
    //! integers per-platform in [`rc`]. The conversation message-array layout is
    //! shared by both (array of pointers); only Solaris differs — see
    //! [`message_at`].

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
            let start_rc = start(&self.service, Some(username), &conv, &mut handle);
            if start_rc != PamReturnCode::SUCCESS || handle.is_null() {
                return Err(PamError::Unavailable(anyhow::anyhow!(
                    "pam_start failed for service `{}`: {:?}",
                    self.service,
                    start_rc
                )));
            }

            // SAFETY: `handle` is non-null per the check above and remains valid
            // until `end`. `creds` outlives the whole transaction.
            let handle_ref = unsafe { &mut *handle };

            // `pam-sys` decodes the raw status with Linux-PAM's numbering, which
            // is wrong on OpenPAM (macOS/BSD). The enum discriminant still equals
            // the raw integer for in-range codes, so recover it with `as i32` and
            // classify against platform-correct constants (see [`rc`]).
            let auth_rc = authenticate(handle_ref, PamFlag::NONE);
            let auth_code = auth_rc as i32;
            let result = if auth_code == rc::SUCCESS {
                let acct_code = acct_mgmt(handle_ref, PamFlag::NONE) as i32;
                if acct_code == rc::SUCCESS {
                    Ok(())
                } else {
                    Err(classify_acct(acct_code))
                }
            } else {
                Err(classify_auth(auth_code))
            };

            end(handle_ref, auth_rc);
            drop(creds);
            result
        }
    }

    /// Index the i-th conversation message. Linux-PAM **and** OpenPAM
    /// (macOS/BSD) pass an array of message *pointers* — message `i` is
    /// `msg[i]` (`*msg.offset(i)`), as OpenPAM's own `openpam_ttyconv` reads it.
    /// Solaris/illumos instead pass a pointer to a single *contiguous* array
    /// (`(*msg).offset(i)`); that is the real PAM portability split. We target
    /// Linux and macOS, so the contiguous form is isolated behind a Solaris cfg
    /// for any future port. (For a single-prompt conversation the two aliasing
    /// at `i == 0` masks the difference; it only bites with multiple messages.)
    #[cfg(not(any(target_os = "solaris", target_os = "illumos")))]
    unsafe fn message_at(msg: *mut *mut PamMessage, i: isize) -> *const PamMessage {
        *msg.offset(i)
    }
    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
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

    /// Raw libpam status codes. `PAM_SUCCESS` is `0` on every platform, but the
    /// *error* codes are numbered differently by Linux-PAM and OpenPAM
    /// (macOS/BSD) — e.g. `PAM_USER_UNKNOWN` is 10 on Linux but 13 on OpenPAM,
    /// where 13 is instead `PAM_ACCT_EXPIRED`. `pam-sys` hard-codes the Linux
    /// numbering, so we re-interpret the raw integer here rather than trusting
    /// its decoded symbol. Sources: `security/_pam_types.h` (Linux),
    /// `security/pam_constants.h` (OpenPAM).
    mod rc {
        pub const SUCCESS: i32 = 0;

        #[cfg(target_os = "linux")]
        mod platform {
            pub const PERM_DENIED: i32 = 6;
            pub const AUTH_ERR: i32 = 7;
            pub const CRED_INSUFFICIENT: i32 = 8;
            pub const USER_UNKNOWN: i32 = 10;
            pub const MAXTRIES: i32 = 11;
            pub const NEW_AUTHTOK_REQD: i32 = 12;
            pub const ACCT_EXPIRED: i32 = 13;
            pub const AUTHTOK_EXPIRED: i32 = 27;
        }

        #[cfg(not(target_os = "linux"))]
        mod platform {
            pub const PERM_DENIED: i32 = 7;
            pub const MAXTRIES: i32 = 8;
            pub const AUTH_ERR: i32 = 9;
            pub const NEW_AUTHTOK_REQD: i32 = 10;
            pub const CRED_INSUFFICIENT: i32 = 11;
            pub const USER_UNKNOWN: i32 = 13;
            pub const ACCT_EXPIRED: i32 = 17;
            pub const AUTHTOK_EXPIRED: i32 = 18;
        }

        pub use platform::*;
    }

    /// Human-readable name for a raw status code, for diagnostics. Falls back to
    /// the bare integer for codes we do not specifically map.
    fn describe(code: i32) -> String {
        let name = match code {
            rc::SUCCESS => "SUCCESS",
            rc::PERM_DENIED => "PERM_DENIED",
            rc::AUTH_ERR => "AUTH_ERR",
            rc::CRED_INSUFFICIENT => "CRED_INSUFFICIENT",
            rc::USER_UNKNOWN => "USER_UNKNOWN",
            rc::MAXTRIES => "MAXTRIES",
            rc::NEW_AUTHTOK_REQD => "NEW_AUTHTOK_REQD",
            rc::ACCT_EXPIRED => "ACCT_EXPIRED",
            rc::AUTHTOK_EXPIRED => "AUTHTOK_EXPIRED",
            _ => return format!("code {code}"),
        };
        format!("{name} ({code})")
    }

    /// Map an `authenticate()` failure: anything that isn't a clear operational
    /// fault is treated as a credential rejection.
    fn classify_auth(code: i32) -> PamError {
        match code {
            rc::AUTH_ERR
            | rc::USER_UNKNOWN
            | rc::CRED_INSUFFICIENT
            | rc::PERM_DENIED
            | rc::MAXTRIES => PamError::AuthFailed,
            other => PamError::Unavailable(anyhow::anyhow!(
                "pam_authenticate failed: {}",
                describe(other)
            )),
        }
    }

    /// Map an `acct_mgmt()` failure: account-state problems are credential-level
    /// rejections; everything else is operational.
    fn classify_acct(code: i32) -> PamError {
        match code {
            rc::ACCT_EXPIRED
            | rc::NEW_AUTHTOK_REQD
            | rc::AUTHTOK_EXPIRED
            | rc::USER_UNKNOWN
            | rc::PERM_DENIED => PamError::AccountInvalid,
            other => {
                PamError::Unavailable(anyhow::anyhow!("pam_acct_mgmt failed: {}", describe(other)))
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn user_unknown_is_a_credential_rejection() {
            // This is the code that surfaced mislabeled as "ACCT_EXPIRED" on
            // macOS (raw 13). It must classify as a 401, not a 500.
            assert!(matches!(classify_auth(rc::USER_UNKNOWN), PamError::AuthFailed));
            assert!(matches!(classify_auth(rc::AUTH_ERR), PamError::AuthFailed));
        }

        #[test]
        fn operational_codes_are_unavailable() {
            // SYSTEM_ERR (4) is the same on both platforms and is operational.
            assert!(matches!(classify_auth(4), PamError::Unavailable(_)));
        }

        #[test]
        fn account_states_are_invalid() {
            assert!(matches!(classify_acct(rc::ACCT_EXPIRED), PamError::AccountInvalid));
            assert!(matches!(classify_acct(rc::NEW_AUTHTOK_REQD), PamError::AccountInvalid));
        }

        #[test]
        fn platform_user_unknown_matches_host_libpam() {
            #[cfg(target_os = "linux")]
            assert_eq!(rc::USER_UNKNOWN, 10);
            #[cfg(not(target_os = "linux"))]
            assert_eq!(rc::USER_UNKNOWN, 13);
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

    // Env-var mutation must be serialized: these tests share process env.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const PAM_VARS: [&str; 3] =
        ["AGENTD_PAM_ENABLED", "AGENTD_PAM_SERVICE", "AGENTD_PAM_EMAIL_DOMAIN"];

    fn clear_pam_env() {
        for v in PAM_VARS {
            std::env::remove_var(v);
        }
    }

    #[test]
    fn file_values_used_when_env_absent() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pam_env();
        let file = agentd_common::config::CorePamConfig {
            enabled: true,
            service: "chkpasswd".to_string(),
            email_domain: "corp.example".to_string(),
        };
        let cfg = PamConfig::from_file_and_env(file);
        clear_pam_env();
        assert!(cfg.enabled);
        assert_eq!(cfg.service, "chkpasswd");
        assert_eq!(cfg.email_domain, "corp.example");
    }

    #[test]
    fn env_overrides_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pam_env();
        std::env::set_var("AGENTD_PAM_ENABLED", "false");
        std::env::set_var("AGENTD_PAM_SERVICE", "agentd");
        let file = agentd_common::config::CorePamConfig {
            enabled: true,
            service: "chkpasswd".to_string(),
            email_domain: "corp.example".to_string(),
        };
        let cfg = PamConfig::from_file_and_env(file);
        clear_pam_env();
        // Env disables even though the file enabled PAM, and overrides the service.
        assert!(!cfg.enabled);
        assert_eq!(cfg.service, "agentd");
        // Untouched field falls through from the file.
        assert_eq!(cfg.email_domain, "corp.example");
    }

    #[test]
    fn blank_env_string_leaves_file_value() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pam_env();
        std::env::set_var("AGENTD_PAM_SERVICE", "   ");
        let file = agentd_common::config::CorePamConfig {
            service: "chkpasswd".to_string(),
            ..Default::default()
        };
        let cfg = PamConfig::from_file_and_env(file);
        clear_pam_env();
        assert_eq!(cfg.service, "chkpasswd");
    }
}
