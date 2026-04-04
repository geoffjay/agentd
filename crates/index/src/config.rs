//! Configuration types for the agentd-index service.
//!
//! Provides [`IndexConfig`] which describes the service configuration and
//! can be loaded from environment variables via [`IndexConfig::from_env`].
//!
//! # Environment Variables
//!
//! | Variable                  | Default                              | Description                    |
//! |---------------------------|--------------------------------------|--------------------------------|
//! | `AGENTD_PORT`             | `17012`                              | HTTP listen port               |
//! | `AGENTD_INDEX_DATA_PATH`  | XDG data dir / `agentd-index`        | Data directory path            |

use std::env;
use std::path::PathBuf;

/// Configuration for the agentd-index service.
///
/// # Example
///
/// ```rust
/// use index::config::IndexConfig;
///
/// let config = IndexConfig::from_env();
/// assert_eq!(config.port, 17012);
/// ```
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// HTTP listen port (default: 17012).
    pub port: u16,

    /// Data directory for index storage.
    pub data_path: PathBuf,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self { port: 17012, data_path: Self::default_data_path() }
    }
}

impl IndexConfig {
    /// Returns the platform-specific default data directory path.
    ///
    /// - **Linux**: `~/.local/share/agentd-index`
    /// - **macOS**: `~/Library/Application Support/agentd-index`
    pub fn default_data_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "agentd-index")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("agentd-index"))
    }

    /// Load configuration from environment variables.
    ///
    /// | Variable                  | Default                        |
    /// |---------------------------|--------------------------------|
    /// | `AGENTD_PORT`             | `17012`                        |
    /// | `AGENTD_INDEX_DATA_PATH`  | XDG data dir / `agentd-index`  |
    pub fn from_env() -> Self {
        let port =
            env::var("AGENTD_PORT").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(17012);

        let data_path = env::var("AGENTD_INDEX_DATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::default_data_path());

        Self { port, data_path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        let config = IndexConfig::default();
        assert_eq!(config.port, 17012);
    }

    #[test]
    fn test_default_data_path_not_empty() {
        let path = IndexConfig::default_data_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_default_data_path_contains_agentd_index() {
        let path = IndexConfig::default_data_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("agentd-index"));
    }

    #[test]
    fn test_from_env_defaults_when_vars_absent() {
        // With no env vars set, should use defaults.
        let config = IndexConfig::from_env();
        assert_eq!(config.port, 17012);
        assert!(!config.data_path.as_os_str().is_empty());
    }

    #[test]
    fn test_clone() {
        let config = IndexConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.port, config.port);
        assert_eq!(cloned.data_path, config.data_path);
    }
}
