//! Runtime smoke tests for the real PAM verifier.
//!
//! These exercise the actual system PAM stack, so they are `#[ignore]` by
//! default and only built with `--features pam`. Run them manually on a host
//! with a PAM service configured:
//!
//! ```bash
//! # macOS: the built-in `chkpasswd` service uses pam_opendirectory.
//! cargo test -p agentd-core --features pam --test pam_smoke -- --ignored --nocapture
//!
//! # To also check a *successful* auth, supply your own password out-of-band:
//! AGENTD_PAM_TEST_USER=$USER AGENTD_PAM_TEST_PASSWORD='...' \
//!   cargo test -p agentd-core --features pam --test pam_smoke -- --ignored --nocapture
//! ```
#![cfg(all(unix, feature = "pam"))]

use agentd_core::pam_auth::build_verifier;

/// Default PAM service to test against: macOS ships `chkpasswd`
/// (pam_opendirectory); Linux hosts should install `/etc/pam.d/agentd`.
fn service() -> String {
    std::env::var("AGENTD_PAM_TEST_SERVICE").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "chkpasswd".to_string()
        } else {
            "agentd".to_string()
        }
    })
}

fn current_user() -> String {
    std::env::var("AGENTD_PAM_TEST_USER")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("LOGNAME"))
        .expect("set USER or AGENTD_PAM_TEST_USER")
}

/// A deliberately wrong password must be rejected — and, crucially, the whole
/// FFI round-trip (pam_start → conversation → pam_authenticate → pam_end) must
/// return cleanly rather than crash. This is the core cross-platform proof.
#[test]
#[ignore = "exercises the real system PAM stack"]
fn wrong_password_is_rejected() {
    let verifier = build_verifier(&service());
    let user = current_user();
    let result = verifier.verify(&user, "definitely-not-the-password-xyzzy");
    assert!(result.is_err(), "a bogus password must not authenticate");
}

/// Optional positive check — only runs when a real password is supplied.
#[test]
#[ignore = "requires AGENTD_PAM_TEST_PASSWORD"]
fn correct_password_authenticates() {
    let Ok(password) = std::env::var("AGENTD_PAM_TEST_PASSWORD") else {
        eprintln!("skipping: AGENTD_PAM_TEST_PASSWORD not set");
        return;
    };
    let verifier = build_verifier(&service());
    let user = current_user();
    verifier.verify(&user, &password).expect("correct password should authenticate");
}
