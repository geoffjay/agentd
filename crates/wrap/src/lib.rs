//! Wrap service library for launching and monitoring agent CLIs.
//!
//! This crate provides both the wrap service implementation and a client library
//! for interacting with it. The service is responsible for launching agents in
//! tmux sessions and monitoring their lifecycle.
//!
//! # Service Implementation
//!
//! The service provides a REST API for launching agent sessions. See `main.rs`
//! for the service entry point.
//!
//! # Client Usage
//!
//! ```no_run
//! use wrap::types::LaunchRequest;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Launch request structure
//! let request = LaunchRequest {
//!     project_name: "my-project".to_string(),
//!     project_path: "/path/to/project".to_string(),
//!     agent_type: "claude-code".to_string(),
//!     model_provider: "anthropic".to_string(),
//!     model_name: "claude-sonnet-4.5".to_string(),
//!     layout: None,
//! };
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod backend;
pub mod client;
#[cfg(feature = "docker")]
pub mod docker;
pub mod tmux;
pub mod types;

pub use backend::{ExecutionBackend, SessionConfig, TmuxBackend};
pub use client::WrapClient;
#[cfg(feature = "docker")]
pub use docker::DockerBackend;
pub use types::{
    HealthResponse, KillSessionResponse, LaunchRequest, LaunchResponse, SessionInfo,
    SessionListResponse, TmuxLayout,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
