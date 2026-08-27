//! Configuration for the agentd-wrap service.
//!
//! All configuration is read from the shared TOML file first, then
//! environment variables are overlaid for backward compatibility.
//!
//! # Environment Variables
//!
//! | Variable                      | Default    | Description                              |
//! |-------------------------------|------------|------------------------------------------|
//! | `AGENTD_HOST`                 | `127.0.0.1`| HTTP bind host                           |
//! | `AGENTD_WRAP_PORT`            | `17005`    | HTTP listen port                         |
//! | `AGENTD_PORT`                 | —          | Fallback port (legacy)                   |
//! | `AGENTD_BACKEND`              | `tmux`     | Execution backend                        |
//! | `AGENTD_WRAP_HISTORY_BYTES`   | `524288`   | PTY ring-buffer size in bytes (512 KiB)  |
//! | `AGENTD_WRAP_CHANNEL_CAPACITY`| `256`      | PTY broadcast channel capacity           |

use agentd_common::config::ValidateConfig;
use anyhow::{bail, Result};
use std::env;

use crate::pty_stream::{DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES};

/// Configuration for the agentd-wrap service.
#[derive(Debug, Clone)]
pub struct WrapConfig {
    /// Bind host (default: `127.0.0.1`)
    pub host: String,
    /// HTTP listen port (default: `17005`)
    pub port: u16,
    /// Execution backend name (default: `"tmux"`)
    pub backend: String,
    /// PTY ring-buffer size in bytes (default: 512 KiB)
    pub history_bytes: usize,
    /// PTY broadcast channel capacity (default: `256`, minimum: `1`)
    pub channel_capacity: usize,
}

impl WrapConfig {
    /// Load configuration from the shared config file and environment variables.
    ///
    /// Reads base values from [`agentd_common::config::load`], then overlays
    /// legacy environment variables for backward compatibility.
    pub fn load() -> Self {
        let shared = agentd_common::config::load().unwrap_or_else(|e| {
            tracing::warn!("failed to load config file, using compiled defaults: {e:#}");
            agentd_common::config::AgentdConfig::default()
        });
        let base = shared.services.wrap;

        let host = env::var("AGENTD_HOST").unwrap_or(shared.general.host);
        let port = env::var("AGENTD_WRAP_PORT")
            .or_else(|_| env::var("AGENTD_PORT"))
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(base.port);
        let backend = env::var("AGENTD_BACKEND").unwrap_or(base.backend);

        // TODO: use base.history_bytes once WrapConfig schema gains this field (#1201)
        let history_bytes = env::var("AGENTD_WRAP_HISTORY_BYTES")
            .ok()
            .and_then(|v| {
                v.parse::<usize>()
                    .map_err(|_| {
                        tracing::warn!(
                            "AGENTD_WRAP_HISTORY_BYTES={v:?} is not a valid usize; \
                             using default {} bytes",
                            DEFAULT_HISTORY_BYTES
                        );
                    })
                    .ok()
            })
            .unwrap_or(DEFAULT_HISTORY_BYTES);

        // TODO: use base.channel_capacity once WrapConfig schema gains this field (#1201)
        let channel_capacity = env::var("AGENTD_WRAP_CHANNEL_CAPACITY")
            .ok()
            .and_then(|v| {
                v.parse::<usize>()
                    .map_err(|_| {
                        tracing::warn!(
                            "AGENTD_WRAP_CHANNEL_CAPACITY={v:?} is not a valid usize; \
                             using default {}",
                            DEFAULT_CHANNEL_CAPACITY
                        );
                    })
                    .ok()
            })
            .unwrap_or(DEFAULT_CHANNEL_CAPACITY);
        let channel_capacity = if channel_capacity == 0 {
            tracing::warn!("AGENTD_WRAP_CHANNEL_CAPACITY=0 is invalid; clamped to 1");
            1
        } else {
            channel_capacity
        };

        Self { host, port, backend, history_bytes, channel_capacity }
    }
}

impl ValidateConfig for WrapConfig {
    fn validate(&self) -> Result<()> {
        if self.port == 0 {
            bail!("wrap.port must be non-zero");
        }
        match self.backend.as_str() {
            "tmux" | "pty" | "subprocess" => {}
            other => {
                bail!("wrap.backend must be one of tmux, pty, subprocess; got: {other}")
            }
        }
        if self.channel_capacity < 1 {
            bail!("wrap.channel_capacity must be at least 1");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Shared with other modules' tests so env-var mutations don't race.
    use crate::ENV_LOCK;

    #[test]
    fn test_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved_host = env::var("AGENTD_HOST").ok();
        let saved_port = env::var("AGENTD_PORT").ok();
        let saved_backend = env::var("AGENTD_BACKEND").ok();
        let saved_cfg = env::var("AGENTD_CONFIG").ok();
        env::remove_var("AGENTD_HOST");
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_BACKEND");
        env::remove_var("AGENTD_WRAP_HISTORY_BYTES");
        env::remove_var("AGENTD_WRAP_CHANNEL_CAPACITY");
        // Point away from the real config file so compiled defaults are used.
        env::set_var("AGENTD_CONFIG", "/tmp/agentd-test-nonexistent-defaults.toml");

        let config = WrapConfig::load();

        if let Some(v) = saved_host {
            env::set_var("AGENTD_HOST", v);
        }
        if let Some(v) = saved_port {
            env::set_var("AGENTD_PORT", v);
        }
        if let Some(v) = saved_backend {
            env::set_var("AGENTD_BACKEND", v);
        }
        match saved_cfg {
            Some(v) => env::set_var("AGENTD_CONFIG", v),
            None => env::remove_var("AGENTD_CONFIG"),
        }

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 17005);
        assert_eq!(config.backend, "tmux");
        assert_eq!(config.history_bytes, DEFAULT_HISTORY_BYTES);
        assert_eq!(config.channel_capacity, DEFAULT_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_channel_capacity_zero_clamped_to_one() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_WRAP_CHANNEL_CAPACITY", "0");
        let config = WrapConfig::load();
        env::remove_var("AGENTD_WRAP_CHANNEL_CAPACITY");
        assert_eq!(config.channel_capacity, 1);
    }

    #[test]
    fn test_env_override() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        env::set_var("AGENTD_PORT", "9005");
        env::set_var("AGENTD_BACKEND", "pty");
        let config = WrapConfig::load();
        env::remove_var("AGENTD_PORT");
        env::remove_var("AGENTD_BACKEND");
        assert_eq!(config.port, 9005);
        assert_eq!(config.backend, "pty");
    }

    #[test]
    fn test_validate_default_passes() {
        let config = WrapConfig {
            host: "127.0.0.1".to_string(),
            port: 17005,
            backend: "tmux".to_string(),
            history_bytes: DEFAULT_HISTORY_BYTES,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_port_fails() {
        let config = WrapConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
            backend: "tmux".to_string(),
            history_bytes: DEFAULT_HISTORY_BYTES,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_backend_fails() {
        let config = WrapConfig {
            host: "127.0.0.1".to_string(),
            port: 17005,
            backend: "unknown".to_string(),
            history_bytes: DEFAULT_HISTORY_BYTES,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn test_validate_all_valid_backends_pass() {
        for backend in &["tmux", "pty", "subprocess"] {
            let config = WrapConfig {
                host: "127.0.0.1".to_string(),
                port: 17005,
                backend: backend.to_string(),
                history_bytes: DEFAULT_HISTORY_BYTES,
                channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            };
            assert!(config.validate().is_ok(), "backend {backend} should be valid");
        }
    }
}
