//! Linear API key configuration.
//!
//! # Configuration
//!
//! The Linear API key can be provided in two ways, checked in this order:
//!
//! ## 1. Environment variable (recommended)
//!
//! Set `AGENTD_LINEAR_API_KEY` to your Linear personal API key:
//!
//! ```sh
//! export AGENTD_LINEAR_API_KEY=lin_api_xxxxxxxxxxxxxxxx
//! ```
//!
//! Personal API keys can be created at:
//! <https://linear.app/settings/api>
//!
//! ## 2. Config file (optional fallback)
//!
//! Add a `[linear]` section to the agentd config file:
//!
//! **Location (checked in order):**
//! - `$AGENTD_CONFIG_FILE` — explicit path override
//! - `~/.config/agentd/config.toml` (Linux / XDG)
//! - `~/Library/Application Support/agentd/config.toml` (macOS)
//!   (uses `directories::ProjectDirs::from("", "", "agentd")`)
//!
//! **Format:**
//! ```toml
//! [linear]
//! api_key = "lin_api_xxxxxxxxxxxxxxxx"
//! ```
//!
//! # Authentication
//!
//! Linear's GraphQL API endpoint is `https://api.linear.app/graphql`.
//! Authentication uses the `Authorization` header with the raw API key value —
//! no `Bearer` prefix is needed for personal API keys:
//!
//! ```text
//! Authorization: lin_api_xxxxxxxxxxxxxxxx
//! ```
//!
//! # Security
//!
//! The API key is **never** logged or included in error messages. All error
//! messages indicate only that a key is missing or present, never the key
//! value itself.

use anyhow::{Context, Result};

/// Configuration for the Linear API integration.
///
/// # Security
///
/// The `api_key` field is intentionally excluded from [`std::fmt::Debug`]
/// output to prevent accidental logging. Use [`LinearConfig::is_configured`]
/// to check availability without exposing the key value.
// `resolve` and `api_key` are not yet called — `LinearIssueSource` (issue #475)
// will use them once the source implementation lands.
#[allow(dead_code)]
pub struct LinearConfig {
    api_key: String,
}

/// Manually implemented to prevent the API key from appearing in log output.
impl std::fmt::Debug for LinearConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearConfig").field("api_key", &"<redacted>").finish()
    }
}

impl LinearConfig {
    /// Resolve the Linear API key from the environment or config file.
    ///
    /// Checks sources in the following order:
    ///
    /// 1. `AGENTD_LINEAR_API_KEY` environment variable
    /// 2. `[linear] api_key` in the agentd config file
    ///
    /// Returns an error if no key is found in either source. The error
    /// message **never includes** the key value.
    // Used by `LinearIssueSource` — arriving in issue #475.
    #[allow(dead_code)]
    pub fn resolve() -> Result<Self> {
        // 1. Environment variable (preferred)
        if let Ok(key) = std::env::var("AGENTD_LINEAR_API_KEY") {
            if !key.trim().is_empty() {
                return Ok(Self { api_key: key });
            }
        }

        // 2. Config file fallback
        if let Some(key) = Self::read_from_config_file()? {
            return Ok(Self { api_key: key });
        }

        anyhow::bail!(
            "Linear API key not configured. \
             Set the AGENTD_LINEAR_API_KEY environment variable \
             or add 'api_key' to the [linear] section of the agentd config file \
             (see documentation for config file location)."
        )
    }

    /// Check whether the Linear API key is available without loading it.
    ///
    /// Returns `true` if `AGENTD_LINEAR_API_KEY` is set to a non-empty value,
    /// or if a key is present in the config file. This is a cheap check
    /// suitable for trigger validation at workflow creation time.
    pub fn is_configured() -> bool {
        if matches!(std::env::var("AGENTD_LINEAR_API_KEY"), Ok(v) if !v.trim().is_empty()) {
            return true;
        }
        match Self::read_from_config_file() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                // Config file exists but could not be read or parsed (e.g. a
                // TOML syntax error). Log a warning so the user knows why their
                // config file is being ignored rather than getting a cryptic
                // "API key not configured" message with no further context.
                tracing::warn!(
                    error = %e,
                    "Failed to read agentd config file while checking Linear API key; \
                     falling back to environment variable only"
                );
                false
            }
        }
    }

    /// Return the API key value.
    ///
    /// # Security
    ///
    /// Do **not** log, format into error messages, or include in API responses.
    // Used by `LinearIssueSource` — arriving in issue #475.
    #[allow(dead_code)]
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Attempt to read the API key from the agentd TOML config file.
    ///
    /// Returns `Ok(None)` if the config file does not exist or does not
    /// contain a `[linear] api_key` entry. Returns `Err` only if the file
    /// exists but cannot be read or parsed.
    fn read_from_config_file() -> Result<Option<String>> {
        let Some(path) = Self::config_file_path()? else {
            return Ok(None);
        };

        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read agentd config file: {}", path.display()))?;

        let value: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse agentd config file: {}", path.display()))?;

        let key = value
            .get("linear")
            .and_then(|section| section.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty());

        Ok(key)
    }

    /// Determine the path to the agentd config file.
    ///
    /// Checked in order:
    /// 1. `AGENTD_CONFIG_FILE` environment variable (explicit override)
    /// 2. Platform config directory via the `directories` crate
    ///    (`ProjectDirs::from("", "", "agentd")`):
    ///    - Linux: `$XDG_CONFIG_HOME/agentd/config.toml`
    ///      (defaults to `~/.config/agentd/config.toml`)
    ///    - macOS: `~/Library/Application Support/agentd/config.toml`
    fn config_file_path() -> Result<Option<std::path::PathBuf>> {
        // Explicit override via environment variable.
        if let Ok(p) = std::env::var("AGENTD_CONFIG_FILE") {
            return Ok(Some(std::path::PathBuf::from(p)));
        }

        // Use empty qualifier and organization so the platform path is simply
        // `<config_dir>/agentd/` on all platforms (e.g. `~/.config/agentd/`
        // on Linux, `~/Library/Application Support/agentd/` on macOS).
        // Using a non-empty organization would add an extra nesting level on
        // macOS: `~/Library/Application Support/<org>/<app>/`.
        let path = directories::ProjectDirs::from("", "", "agentd")
            .map(|d| d.config_dir().join("config.toml"));

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Test helpers
    // ---------------------------------------------------------------------------

    /// Global mutex to serialize env-var-mutating tests.
    ///
    /// Rust runs unit tests in parallel by default. Because environment
    /// variables are process-wide, tests that set/unset `AGENTD_LINEAR_API_KEY`
    /// or `AGENTD_CONFIG_FILE` must not overlap. Each test acquires this lock
    /// once via [`with_env_lock`] for the duration of its body.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run a test closure while holding `ENV_LOCK`, ensuring env-var mutations
    /// from other tests don't race with this one.
    fn with_env_lock(f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        f();
    }

    /// RAII guard that restores a single env var to its previous value on drop.
    /// Must be used inside `with_env_lock`.
    struct EnvRestorer {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestorer {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestorer {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => std::env::set_var(self.key, val),
                None => std::env::remove_var(self.key),
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[test]
    fn is_configured_returns_false_when_env_unset() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let tmp = std::env::temp_dir().join("agentd_linear_test_nonexistent.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());
            assert!(!LinearConfig::is_configured());
        });
    }

    #[test]
    fn is_configured_returns_true_when_env_set() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_testkey");
            assert!(LinearConfig::is_configured());
        });
    }

    #[test]
    fn resolve_succeeds_when_env_set() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_testkey123");
            let cfg = LinearConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.api_key(), "lin_api_testkey123");
        });
    }

    #[test]
    fn resolve_fails_when_no_key_available() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let tmp = std::env::temp_dir().join("agentd_linear_test_nonexistent2.toml");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &tmp.to_string_lossy());

            let err = LinearConfig::resolve().expect_err("should fail without key");
            let msg = err.to_string();
            assert!(msg.contains("AGENTD_LINEAR_API_KEY"), "message: {msg}");
            assert!(!msg.contains("lin_api_"), "key must not appear in error: {msg}");
        });
    }

    #[test]
    fn resolve_reads_key_from_config_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[linear]\napi_key = \"lin_api_fromfile\"\n")
                .expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let cfg = LinearConfig::resolve().expect("should resolve from file");
            assert_eq!(cfg.api_key(), "lin_api_fromfile");
        });
    }

    #[test]
    fn resolve_env_takes_precedence_over_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::set("AGENTD_LINEAR_API_KEY", "lin_api_fromenv");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[linear]\napi_key = \"lin_api_fromfile\"\n")
                .expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let cfg = LinearConfig::resolve().expect("should resolve from env");
            assert_eq!(cfg.api_key(), "lin_api_fromenv");
        });
    }

    #[test]
    fn config_file_missing_linear_section_returns_none() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            std::fs::write(&path, "[github]\ntoken = \"gh_token\"\n").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            let err = LinearConfig::resolve().expect_err("should fail — no linear key");
            assert!(err.to_string().contains("AGENTD_LINEAR_API_KEY"));
        });
    }

    #[test]
    fn is_configured_returns_false_for_malformed_config_file() {
        with_env_lock(|| {
            let _api_key = EnvRestorer::unset("AGENTD_LINEAR_API_KEY");
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("config.toml");
            // Write deliberately invalid TOML.
            std::fs::write(&path, "this is [not valid toml syntax !!!").expect("write config");
            let _cfg = EnvRestorer::set("AGENTD_CONFIG_FILE", &path.to_string_lossy());

            // is_configured() must not panic; it logs a warning and returns false.
            assert!(!LinearConfig::is_configured());
            // resolve() must surface the parse error, not a generic "key not configured" message.
            let err = LinearConfig::resolve().expect_err("should fail — malformed config");
            let msg = err.to_string();
            assert!(msg.contains("parse") || msg.contains("config file"), "message: {msg}");
        });
    }
}
