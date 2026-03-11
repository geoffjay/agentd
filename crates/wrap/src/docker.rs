//! Docker execution backend for agent session management.
//!
//! This module implements the [`ExecutionBackend`] trait using the
//! [bollard](https://docs.rs/bollard) crate to manage Docker containers
//! as isolated agent execution environments.
//!
//! Each agent gets its own container with the appropriate CLI as the
//! entrypoint. The container connects back to the orchestrator's WebSocket
//! endpoint for streaming output.
//!
//! # Examples
//!
//! ```no_run
//! use wrap::docker::DockerBackend;
//! use wrap::backend::{ExecutionBackend, SessionConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let backend = DockerBackend::new("agentd", "agentd-claude:latest")?;
//!
//! let config = SessionConfig {
//!     session_name: "agentd-abc123".into(),
//!     working_dir: "/home/user/project".into(),
//!     agent_type: "claude-code".into(),
//!     model_provider: "anthropic".into(),
//!     model_name: "claude-sonnet-4.5".into(),
//!     layout: None,
//!     network_policy: None,
//! };
//!
//! backend.create_session(&config).await?;
//! backend.launch_agent(&config).await?;
//!
//! assert!(backend.session_exists(&config.session_name).await?);
//!
//! backend.kill_session(&config.session_name).await?;
//! # Ok(())
//! # }
//! ```

use crate::backend::{ExecutionBackend, SessionConfig};
use async_trait::async_trait;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::exec::CreateExecOptions;
use bollard::models::ContainerStateStatusEnum;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Default container image used when none is specified.
pub const DEFAULT_IMAGE: &str = "agentd-claude:latest";

/// Default memory limit for agent containers (2 GiB).
const DEFAULT_MEMORY_BYTES: i64 = 2 * 1024 * 1024 * 1024;

/// Default CPU limit for agent containers (2 CPUs in nano-CPUs).
const DEFAULT_NANO_CPUS: i64 = 2_000_000_000;

/// Graceful stop timeout before SIGKILL (seconds).
const STOP_TIMEOUT_SECS: i64 = 10;

/// Label key for the backend prefix (used to filter containers).
const LABEL_PREFIX: &str = "agentd.prefix";

/// Label key for the agent ID.
const LABEL_AGENT_ID: &str = "agentd.agent-id";

/// Label key for the session name.
const LABEL_SESSION: &str = "agentd.session";

/// Well-known API key environment variables forwarded into containers.
const FORWARDED_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "ANTHROPIC_BASE_URL",
    "OPENAI_BASE_URL",
];

/// Docker host gateway hostname used by Docker Desktop (macOS/Windows) and
/// Docker Engine 20.10+ (Linux with `--add-host` or host-gateway support).
const DOCKER_HOST_INTERNAL: &str = "host.docker.internal";

/// The host gateway IP that Docker maps `host.docker.internal` to.
const HOST_GATEWAY: &str = "host-gateway";

// ---------------------------------------------------------------------------
// NetworkPolicy
// ---------------------------------------------------------------------------

/// Network access policy for Docker-backed agent containers.
///
/// Controls whether a container has internet access, no network at all,
/// or shares the host's network stack. The policy also determines how
/// the container reaches the orchestrator's WebSocket endpoint.
///
/// # Platform notes
///
/// - **macOS / Windows (Docker Desktop)**: `host.docker.internal` is
///   automatically available. `HostNetwork` is not supported on macOS.
/// - **Linux (Docker Engine)**: `host.docker.internal` requires Docker
///   20.10+ or an explicit `--add-host` entry. `HostNetwork` gives the
///   container the host's full network stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// Bridge network with outbound internet access (default).
    ///
    /// The container can reach the internet and connects back to the
    /// orchestrator via `host.docker.internal`.
    #[default]
    Internet,

    /// No network access at all (`--network none`).
    ///
    /// The container cannot make any network connections, including to
    /// the host. A dedicated Docker network is created to allow only
    /// the WebSocket connection back to the orchestrator.
    ///
    /// **Note**: Because `--network none` removes all network
    /// interfaces, isolated containers use a dedicated `agentd-isolated`
    /// network that only provides a route to the host gateway. No
    /// outbound internet routes are configured.
    Isolated,

    /// Host network mode (`--network host`).
    ///
    /// The container shares the host's network stack. `127.0.0.1`
    /// reaches the host directly. This is the simplest configuration
    /// on Linux but is **not supported on macOS/Windows**.
    HostNetwork,
}

impl NetworkPolicy {
    /// Returns the Docker `network_mode` string for this policy.
    pub fn to_network_mode(&self) -> &'static str {
        match self {
            NetworkPolicy::Internet => "bridge",
            NetworkPolicy::Isolated => "bridge",
            NetworkPolicy::HostNetwork => "host",
        }
    }
}

impl std::fmt::Display for NetworkPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkPolicy::Internet => write!(f, "internet"),
            NetworkPolicy::Isolated => write!(f, "isolated"),
            NetworkPolicy::HostNetwork => write!(f, "host_network"),
        }
    }
}

impl std::str::FromStr for NetworkPolicy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "internet" => Ok(NetworkPolicy::Internet),
            "isolated" => Ok(NetworkPolicy::Isolated),
            "host_network" => Ok(NetworkPolicy::HostNetwork),
            _ => Err(anyhow::anyhow!("Unknown network policy: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceLimits
// ---------------------------------------------------------------------------

/// Configuration for resource limits on agent containers.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Memory limit in bytes. Defaults to 2 GiB.
    pub memory_bytes: i64,
    /// CPU limit in nano-CPUs (1e9 = 1 CPU). Defaults to 2 CPUs.
    pub nano_cpus: i64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self { memory_bytes: DEFAULT_MEMORY_BYTES, nano_cpus: DEFAULT_NANO_CPUS }
    }
}

/// Docker-based execution backend for running agents in containers.
///
/// Each agent session maps to a Docker container. The container is created
/// with `create_session` and started with `launch_agent`. Container labels
/// are used to track which containers belong to this backend instance.
///
/// # Container naming
///
/// Containers are named using the `session_name` from [`SessionConfig`],
/// which typically has the form `{prefix}-{agent_id}`.
///
/// # Networking
///
/// The [`NetworkPolicy`] controls how containers reach the network and
/// the orchestrator's WebSocket endpoint:
///
/// - **`Internet`** (default): bridge network with outbound access;
///   reaches the host via `host.docker.internal`.
/// - **`Isolated`**: bridge network with no outbound internet; only the
///   host gateway is reachable for WebSocket connectivity.
/// - **`HostNetwork`**: `--network host` (Linux only); `127.0.0.1`
///   reaches the host directly.
pub struct DockerBackend {
    /// Session name prefix (e.g., `"agentd-orch"`).
    prefix: String,
    /// Container image to use (e.g., `"agentd-claude:latest"`).
    image: String,
    /// Network access policy for containers.
    network_policy: NetworkPolicy,
    /// Optional resource limits for agent containers.
    resource_limits: ResourceLimits,
    /// The orchestrator port containers connect back to.
    orchestrator_port: u16,
    /// Bollard Docker client.
    docker: Docker,
}

/// Default orchestrator port for WebSocket connections.
const DEFAULT_ORCHESTRATOR_PORT: u16 = 7006;

impl DockerBackend {
    /// Creates a new `DockerBackend` with the given prefix and image.
    ///
    /// Connects to the Docker daemon using the `DOCKER_HOST` environment
    /// variable if set, otherwise uses the platform default socket.
    ///
    /// Uses [`NetworkPolicy::Internet`] by default.
    ///
    /// # Errors
    ///
    /// Returns an error if the Docker client cannot be initialized (e.g.,
    /// invalid `DOCKER_HOST` value).
    pub fn new(prefix: impl Into<String>, image: impl Into<String>) -> anyhow::Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self {
            prefix: prefix.into(),
            image: image.into(),
            network_policy: NetworkPolicy::default(),
            resource_limits: ResourceLimits::default(),
            orchestrator_port: DEFAULT_ORCHESTRATOR_PORT,
            docker,
        })
    }

    /// Creates a new `DockerBackend` with full configuration control.
    ///
    /// # Arguments
    ///
    /// * `prefix` — Session name prefix.
    /// * `image` — Container image name.
    /// * `network_policy` — Network access policy for containers.
    /// * `resource_limits` — CPU and memory limits for containers.
    pub fn with_config(
        prefix: impl Into<String>,
        image: impl Into<String>,
        network_policy: NetworkPolicy,
        resource_limits: ResourceLimits,
    ) -> anyhow::Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self {
            prefix: prefix.into(),
            image: image.into(),
            network_policy,
            resource_limits,
            orchestrator_port: DEFAULT_ORCHESTRATOR_PORT,
            docker,
        })
    }

    /// Sets the orchestrator port for WebSocket URL generation.
    pub fn with_orchestrator_port(mut self, port: u16) -> Self {
        self.orchestrator_port = port;
        self
    }

    /// Returns the configured container image.
    pub fn image(&self) -> &str {
        &self.image
    }

    /// Returns the configured network policy.
    pub fn network_policy(&self) -> &NetworkPolicy {
        &self.network_policy
    }

    /// Returns the configured resource limits.
    pub fn resource_limits(&self) -> &ResourceLimits {
        &self.resource_limits
    }

    /// Returns the orchestrator port used for WebSocket URLs.
    pub fn orchestrator_port(&self) -> u16 {
        self.orchestrator_port
    }

    /// Extract the agent ID from a session name.
    ///
    /// Session names follow the pattern `{prefix}-{agent_id}`. This
    /// extracts the agent ID portion after the prefix.
    fn extract_agent_id(&self, session_name: &str) -> String {
        session_name
            .strip_prefix(&self.prefix)
            .and_then(|s| s.strip_prefix('-'))
            .unwrap_or(session_name)
            .to_string()
    }

    /// Build the list of environment variables for a container.
    ///
    /// Includes agent metadata and forwards well-known API keys from
    /// the host environment.
    fn build_container_env(&self, config: &SessionConfig) -> Vec<String> {
        let mut env = vec![
            format!("AGENT_TYPE={}", config.agent_type),
            format!("MODEL_PROVIDER={}", config.model_provider),
            format!("MODEL_NAME={}", config.model_name),
        ];

        // Forward well-known API keys from the host environment.
        for key in FORWARDED_ENV_KEYS {
            if let Ok(val) = std::env::var(key) {
                env.push(format!("{key}={val}"));
            }
        }

        env
    }

    /// Build the container labels for tracking and filtering.
    fn build_labels(&self, config: &SessionConfig) -> HashMap<String, String> {
        let mut labels = HashMap::new();
        labels.insert(LABEL_PREFIX.to_string(), self.prefix.clone());
        labels.insert(LABEL_SESSION.to_string(), config.session_name.clone());
        labels.insert(LABEL_AGENT_ID.to_string(), self.extract_agent_id(&config.session_name));
        labels
    }
}

/// Build the container command (CMD) for the given agent type.
///
/// This mirrors [`build_agent_command`](crate::backend) but returns a
/// `Vec<String>` suitable for Docker's `Cmd` field.
fn build_agent_cmd(config: &SessionConfig) -> anyhow::Result<Vec<String>> {
    match config.agent_type.as_str() {
        "claude-code" => Ok(vec!["claude".to_string()]),
        "crush" => Ok(vec!["crush".to_string()]),
        "opencode" => Ok(vec![
            "opencode".to_string(),
            "--model-provider".to_string(),
            config.model_provider.clone(),
            "--model".to_string(),
            config.model_name.clone(),
        ]),
        "gemini" => {
            Ok(vec!["gemini".to_string(), "--model".to_string(), config.model_name.clone()])
        }
        "general" => Ok(vec!["/bin/bash".to_string()]),
        other => Err(anyhow::anyhow!("Unsupported agent type: {}", other)),
    }
}

/// Check if a bollard error is a 404 Not Found response.
fn is_not_found(err: &bollard::errors::Error) -> bool {
    matches!(err, bollard::errors::Error::DockerResponseServerError { status_code: 404, .. })
}

/// Check if a bollard error is a 304 Not Modified response (container already stopped).
fn is_not_modified(err: &bollard::errors::Error) -> bool {
    matches!(err, bollard::errors::Error::DockerResponseServerError { status_code: 304, .. })
}

/// Check if a bollard error is a 409 Conflict response (container already exists).
fn is_conflict(err: &bollard::errors::Error) -> bool {
    matches!(err, bollard::errors::Error::DockerResponseServerError { status_code: 409, .. })
}

#[async_trait]
impl ExecutionBackend for DockerBackend {
    /// Creates a Docker container for the agent session.
    ///
    /// The container is created but **not started** — call [`launch_agent`]
    /// to start it. The container includes:
    ///
    /// - A volume mount mapping `working_dir` to `/workspace`
    /// - Environment variables for agent metadata and forwarded API keys
    /// - Labels for filtering and tracking
    /// - Resource limits (CPU and memory)
    /// - Network configuration based on the [`NetworkPolicy`]
    /// - DNS configuration (Docker default for `Internet`, none for `Isolated`)
    /// - Non-root user (`1000:1000`)
    async fn create_session(&self, config: &SessionConfig) -> anyhow::Result<()> {
        let cmd = build_agent_cmd(config)?;
        let env = self.build_container_env(config);
        let labels = self.build_labels(config);

        // Resolve the network policy — use the session-level override if
        // present, otherwise fall back to the backend default.
        let effective_policy = config.network_policy.as_ref().unwrap_or(&self.network_policy);

        let network_mode = effective_policy.to_network_mode().to_string();

        // Build host config with network policy applied.
        let mut host_config = bollard::models::HostConfig {
            binds: Some(vec![format!("{}:/workspace:rw", config.working_dir)]),
            network_mode: Some(network_mode.clone()),
            memory: Some(self.resource_limits.memory_bytes),
            nano_cpus: Some(self.resource_limits.nano_cpus),
            ..Default::default()
        };

        // On Linux, `host.docker.internal` is not always available.
        // Add an explicit extra host entry so bridge/isolated containers
        // can always reach the host. On macOS/Windows (Docker Desktop)
        // this is already available but the extra entry is harmless.
        if *effective_policy != NetworkPolicy::HostNetwork {
            host_config.extra_hosts =
                Some(vec![format!("{}:{}", DOCKER_HOST_INTERNAL, HOST_GATEWAY)]);
        }

        // For isolated containers, drop all capabilities and block
        // outbound traffic by not providing DNS servers. The container
        // can still reach the host gateway for the WebSocket connection.
        if *effective_policy == NetworkPolicy::Isolated {
            host_config.dns = Some(vec![]);
            info!(
                session = %config.session_name,
                "Creating isolated container (no outbound internet)"
            );
        }

        let create_opts =
            CreateContainerOptions { name: config.session_name.clone(), ..Default::default() };

        let container_config = Config {
            image: Some(self.image.clone()),
            cmd: Some(cmd),
            working_dir: Some("/workspace".to_string()),
            env: Some(env),
            labels: Some(labels),
            host_config: Some(host_config),
            user: Some("1000:1000".to_string()),
            ..Default::default()
        };

        match self.docker.create_container(Some(create_opts), container_config).await {
            Ok(response) => {
                info!(
                    session = %config.session_name,
                    container_id = %response.id,
                    image = %self.image,
                    network_policy = %effective_policy,
                    "Docker container created"
                );

                // Log any warnings from the Docker daemon.
                for w in &response.warnings {
                    warn!(session = %config.session_name, warning = %w, "Docker create warning");
                }

                Ok(())
            }
            Err(e) if is_conflict(&e) => {
                // Container already exists — this is acceptable for idempotency.
                debug!(
                    session = %config.session_name,
                    "Container already exists, skipping create"
                );
                Ok(())
            }
            Err(e) => {
                Err(anyhow::anyhow!("Failed to create container '{}': {}", config.session_name, e))
            }
        }
    }

    /// Starts a previously created Docker container.
    ///
    /// The container's CMD (set during `create_session`) determines which
    /// agent CLI is launched.
    async fn launch_agent(&self, config: &SessionConfig) -> anyhow::Result<()> {
        self.docker
            .start_container(&config.session_name, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to start container '{}': {}", config.session_name, e)
            })?;

        info!(
            session = %config.session_name,
            "Docker container started"
        );

        Ok(())
    }

    /// Checks whether a container exists and is in a running or created state.
    ///
    /// Returns `false` for containers that have exited, been removed, or
    /// never existed.
    async fn session_exists(&self, session_name: &str) -> anyhow::Result<bool> {
        match self.docker.inspect_container(session_name, None).await {
            Ok(info) => {
                let exists = info
                    .state
                    .as_ref()
                    .map(|state| {
                        let running = state.running.unwrap_or(false);
                        let is_created = state.status == Some(ContainerStateStatusEnum::CREATED);
                        running || is_created
                    })
                    .unwrap_or(false);

                debug!(
                    session = %session_name,
                    exists,
                    "Container existence check"
                );

                Ok(exists)
            }
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("Failed to inspect container '{}': {}", session_name, e)),
        }
    }

    /// Stops and removes a Docker container.
    ///
    /// This is idempotent — calling it on a non-existent or already-stopped
    /// container is not an error. The container is first stopped with a
    /// graceful timeout, then force-removed.
    async fn kill_session(&self, session_name: &str) -> anyhow::Result<()> {
        // Stop the container with a graceful timeout.
        let stop_opts = StopContainerOptions { t: STOP_TIMEOUT_SECS };
        match self.docker.stop_container(session_name, Some(stop_opts)).await {
            Ok(_) => {
                debug!(session = %session_name, "Container stopped");
            }
            Err(e) if is_not_found(&e) || is_not_modified(&e) => {
                // Container doesn't exist or is already stopped — fine.
                debug!(session = %session_name, "Container already stopped or not found");
            }
            Err(e) => {
                warn!(session = %session_name, %e, "Error stopping container, attempting removal anyway");
            }
        }

        // Force-remove the container (removes even if stop failed).
        let rm_opts = RemoveContainerOptions { force: true, ..Default::default() };
        match self.docker.remove_container(session_name, Some(rm_opts)).await {
            Ok(_) => {
                info!(session = %session_name, "Container removed");
            }
            Err(e) if is_not_found(&e) => {
                debug!(session = %session_name, "Container already removed");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to remove container '{}': {}",
                    session_name,
                    e
                ));
            }
        }

        Ok(())
    }

    /// Sends an arbitrary command into a running container via `docker exec`.
    ///
    /// The command is executed as a shell command (`/bin/sh -c "<command>"`)
    /// inside the container. This is used by the orchestrator to inject
    /// fully-constructed CLI commands (with `--sdk-url`, env vars, etc.)
    /// into an already-running session.
    async fn send_command(&self, session_name: &str, command: &str) -> anyhow::Result<()> {
        let exec_opts = CreateExecOptions {
            cmd: Some(vec!["/bin/sh", "-c", command]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self.docker.create_exec(session_name, exec_opts).await.map_err(|e| {
            anyhow::anyhow!("Failed to create exec in container '{}': {}", session_name, e)
        })?;

        self.docker.start_exec(&exec.id, None).await.map_err(|e| {
            anyhow::anyhow!("Failed to start exec in container '{}': {}", session_name, e)
        })?;

        debug!(
            session = %session_name,
            "Command sent to container via exec"
        );

        Ok(())
    }

    /// Lists all containers managed by this backend instance.
    ///
    /// Filters by the `agentd.prefix` label to find containers belonging
    /// to this backend. Returns container names (without the leading `/`).
    async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        let label_filter = format!("{}={}", LABEL_PREFIX, self.prefix);
        let mut filters = HashMap::new();
        filters.insert("label", vec![label_filter.as_str()]);

        let opts = ListContainersOptions { all: true, filters, ..Default::default() };

        let containers = self
            .docker
            .list_containers(Some(opts))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list containers: {}", e))?;

        let names: Vec<String> = containers
            .iter()
            .filter_map(|c| {
                c.names.as_ref()?.first().map(|n| n.trim_start_matches('/').to_string())
            })
            .collect();

        debug!(count = names.len(), prefix = %self.prefix, "Listed Docker sessions");

        Ok(names)
    }

    fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns a WebSocket URL that containers can use to reach the host.
    ///
    /// The URL is determined by the [`NetworkPolicy`]:
    ///
    /// - **`HostNetwork`**: `ws://127.0.0.1:{port}/ws/{id}` — the
    ///   container shares the host's network stack.
    /// - **`Internet` / `Isolated`**: `ws://host.docker.internal:{port}/ws/{id}`
    ///   — the container uses Docker's host gateway to reach the host.
    ///   An extra-hosts entry is added during `create_session` to ensure
    ///   `host.docker.internal` resolves on all platforms.
    fn agent_ws_url(&self, session_name: &str) -> Option<String> {
        let agent_id = self.extract_agent_id(session_name);
        let port = self.orchestrator_port;

        match self.network_policy {
            NetworkPolicy::HostNetwork => {
                // Host networking — container shares the host's network stack.
                Some(format!("ws://127.0.0.1:{port}/ws/{agent_id}"))
            }
            NetworkPolicy::Internet | NetworkPolicy::Isolated => {
                // Bridge networking — use Docker's host gateway.
                Some(format!("ws://{DOCKER_HOST_INTERNAL}:{port}/ws/{agent_id}"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TmuxLayout;

    /// Helper to construct a `DockerBackend` for unit tests without requiring
    /// a running Docker daemon. Uses `Docker::connect_with_local_defaults()`
    /// which only fails if the platform has no default socket path at all.
    fn test_backend() -> DockerBackend {
        // Construct directly to avoid requiring a running daemon for unit tests.
        DockerBackend {
            prefix: "test-prefix".to_string(),
            image: "agentd-claude:latest".to_string(),
            network_policy: NetworkPolicy::Internet,
            resource_limits: ResourceLimits::default(),
            orchestrator_port: DEFAULT_ORCHESTRATOR_PORT,
            docker: Docker::connect_with_local_defaults()
                .expect("Docker client construction should not fail"),
        }
    }

    fn test_session_config() -> SessionConfig {
        SessionConfig {
            session_name: "test-prefix-abc123".into(),
            working_dir: "/tmp/test-project".into(),
            agent_type: "claude-code".into(),
            model_provider: "anthropic".into(),
            model_name: "claude-sonnet-4.5".into(),
            layout: None,
            network_policy: None,
        }
    }

    // -- DockerBackend construction and accessors --

    #[test]
    fn docker_backend_prefix() {
        let backend = test_backend();
        assert_eq!(backend.prefix(), "test-prefix");
    }

    #[test]
    fn docker_backend_image() {
        let backend = test_backend();
        assert_eq!(backend.image(), "agentd-claude:latest");
    }

    #[test]
    fn docker_backend_network_policy() {
        let backend = test_backend();
        assert_eq!(*backend.network_policy(), NetworkPolicy::Internet);
    }

    #[test]
    fn docker_backend_default_resource_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.memory_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(limits.nano_cpus, 2_000_000_000);
    }

    // -- Object safety and Send + Sync --

    #[test]
    fn docker_backend_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<DockerBackend>();
    }

    #[test]
    fn trait_is_object_safe_with_docker() {
        fn _assert_object_safe(_: &dyn ExecutionBackend) {}
        let backend = test_backend();
        _assert_object_safe(&backend);
    }

    // -- build_agent_cmd --

    #[test]
    fn build_agent_cmd_claude_code() {
        let config = test_session_config();
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["claude"]);
    }

    #[test]
    fn build_agent_cmd_crush() {
        let config = SessionConfig { agent_type: "crush".into(), ..test_session_config() };
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["crush"]);
    }

    #[test]
    fn build_agent_cmd_opencode() {
        let config = SessionConfig {
            agent_type: "opencode".into(),
            model_provider: "openai".into(),
            model_name: "gpt-4".into(),
            ..test_session_config()
        };
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["opencode", "--model-provider", "openai", "--model", "gpt-4"]);
    }

    #[test]
    fn build_agent_cmd_gemini() {
        let config = SessionConfig {
            agent_type: "gemini".into(),
            model_provider: "google".into(),
            model_name: "gemini-pro".into(),
            ..test_session_config()
        };
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["gemini", "--model", "gemini-pro"]);
    }

    #[test]
    fn build_agent_cmd_general() {
        let config = SessionConfig { agent_type: "general".into(), ..test_session_config() };
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["/bin/bash"]);
    }

    #[test]
    fn build_agent_cmd_unsupported() {
        let config = SessionConfig { agent_type: "unknown".into(), ..test_session_config() };
        let err = build_agent_cmd(&config).unwrap_err();
        assert!(err.to_string().contains("Unsupported agent type"));
    }

    // -- agent_ws_url --

    #[test]
    fn agent_ws_url_bridge_mode() {
        let backend = test_backend();
        let url = backend.agent_ws_url("test-prefix-abc123");
        assert_eq!(url, Some("ws://host.docker.internal:7006/ws/abc123".to_string()));
    }

    #[test]
    fn agent_ws_url_host_mode() {
        let mut backend = test_backend();
        backend.network_policy = NetworkPolicy::HostNetwork;
        let url = backend.agent_ws_url("test-prefix-abc123");
        assert_eq!(url, Some("ws://127.0.0.1:7006/ws/abc123".to_string()));
    }

    #[test]
    fn agent_ws_url_returns_some() {
        let backend = test_backend();
        assert!(backend.agent_ws_url("any-session").is_some());
    }

    // -- extract_agent_id --

    #[test]
    fn extract_agent_id_with_prefix() {
        let backend = test_backend();
        assert_eq!(backend.extract_agent_id("test-prefix-abc123"), "abc123");
    }

    #[test]
    fn extract_agent_id_without_prefix() {
        let backend = test_backend();
        assert_eq!(backend.extract_agent_id("other-prefix-xyz"), "other-prefix-xyz");
    }

    #[test]
    fn extract_agent_id_uuid_format() {
        let backend = test_backend();
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let session = format!("test-prefix-{}", id);
        assert_eq!(backend.extract_agent_id(&session), id);
    }

    // -- build_container_env --

    #[test]
    fn build_container_env_includes_metadata() {
        let backend = test_backend();
        let config = test_session_config();
        let env = backend.build_container_env(&config);

        assert!(env.contains(&"AGENT_TYPE=claude-code".to_string()));
        assert!(env.contains(&"MODEL_PROVIDER=anthropic".to_string()));
        assert!(env.contains(&"MODEL_NAME=claude-sonnet-4.5".to_string()));
    }

    // -- build_labels --

    #[test]
    fn build_labels_sets_all_keys() {
        let backend = test_backend();
        let config = test_session_config();
        let labels = backend.build_labels(&config);

        assert_eq!(labels.get(LABEL_PREFIX), Some(&"test-prefix".to_string()));
        assert_eq!(labels.get(LABEL_SESSION), Some(&"test-prefix-abc123".to_string()));
        assert_eq!(labels.get(LABEL_AGENT_ID), Some(&"abc123".to_string()));
    }

    // -- ResourceLimits --

    #[test]
    fn resource_limits_custom() {
        let limits =
            ResourceLimits { memory_bytes: 4 * 1024 * 1024 * 1024, nano_cpus: 4_000_000_000 };
        assert_eq!(limits.memory_bytes, 4 * 1024 * 1024 * 1024);
        assert_eq!(limits.nano_cpus, 4_000_000_000);
    }

    #[test]
    fn resource_limits_clone() {
        let limits = ResourceLimits::default();
        let cloned = limits.clone();
        assert_eq!(limits.memory_bytes, cloned.memory_bytes);
        assert_eq!(limits.nano_cpus, cloned.nano_cpus);
    }

    // -- Error helpers --

    #[test]
    fn is_not_found_detects_404() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            message: "not found".to_string(),
        };
        assert!(is_not_found(&err));
        assert!(!is_not_modified(&err));
        assert!(!is_conflict(&err));
    }

    #[test]
    fn is_not_modified_detects_304() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 304,
            message: "not modified".to_string(),
        };
        assert!(is_not_modified(&err));
        assert!(!is_not_found(&err));
    }

    #[test]
    fn is_conflict_detects_409() {
        let err = bollard::errors::Error::DockerResponseServerError {
            status_code: 409,
            message: "conflict".to_string(),
        };
        assert!(is_conflict(&err));
        assert!(!is_not_found(&err));
    }

    // -- SessionConfig with layout is ignored --

    #[test]
    fn session_config_layout_ignored_by_docker() {
        let config = SessionConfig {
            layout: Some(TmuxLayout { layout_type: "vertical".into(), panes: Some(2) }),
            ..test_session_config()
        };
        // Docker backend should still produce a valid command regardless of layout.
        let cmd = build_agent_cmd(&config).unwrap();
        assert_eq!(cmd, vec!["claude"]);
    }

    // -- NetworkPolicy --

    #[test]
    fn network_policy_default_is_internet() {
        let policy = NetworkPolicy::default();
        assert_eq!(policy, NetworkPolicy::Internet);
    }

    #[test]
    fn network_policy_to_network_mode() {
        assert_eq!(NetworkPolicy::Internet.to_network_mode(), "bridge");
        assert_eq!(NetworkPolicy::Isolated.to_network_mode(), "bridge");
        assert_eq!(NetworkPolicy::HostNetwork.to_network_mode(), "host");
    }

    #[test]
    fn network_policy_display() {
        assert_eq!(NetworkPolicy::Internet.to_string(), "internet");
        assert_eq!(NetworkPolicy::Isolated.to_string(), "isolated");
        assert_eq!(NetworkPolicy::HostNetwork.to_string(), "host_network");
    }

    #[test]
    fn network_policy_from_str() {
        assert_eq!("internet".parse::<NetworkPolicy>().unwrap(), NetworkPolicy::Internet);
        assert_eq!("isolated".parse::<NetworkPolicy>().unwrap(), NetworkPolicy::Isolated);
        assert_eq!("host_network".parse::<NetworkPolicy>().unwrap(), NetworkPolicy::HostNetwork);
        assert!("invalid".parse::<NetworkPolicy>().is_err());
    }

    #[test]
    fn network_policy_serde_roundtrip() {
        for policy in [NetworkPolicy::Internet, NetworkPolicy::Isolated, NetworkPolicy::HostNetwork]
        {
            let json = serde_json::to_string(&policy).unwrap();
            let deserialized: NetworkPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, policy);
        }
    }

    #[test]
    fn network_policy_serde_values() {
        assert_eq!(serde_json::to_string(&NetworkPolicy::Internet).unwrap(), "\"internet\"");
        assert_eq!(serde_json::to_string(&NetworkPolicy::Isolated).unwrap(), "\"isolated\"");
        assert_eq!(serde_json::to_string(&NetworkPolicy::HostNetwork).unwrap(), "\"host_network\"");
    }

    // -- agent_ws_url with NetworkPolicy --

    #[test]
    fn agent_ws_url_internet_mode() {
        let backend = test_backend(); // Internet policy by default
        let url = backend.agent_ws_url("test-prefix-abc123");
        assert_eq!(url, Some("ws://host.docker.internal:7006/ws/abc123".to_string()));
    }

    #[test]
    fn agent_ws_url_isolated_mode() {
        let mut backend = test_backend();
        backend.network_policy = NetworkPolicy::Isolated;
        let url = backend.agent_ws_url("test-prefix-abc123");
        // Isolated still needs host gateway for WebSocket.
        assert_eq!(url, Some("ws://host.docker.internal:7006/ws/abc123".to_string()));
    }

    #[test]
    fn agent_ws_url_custom_port() {
        let mut backend = test_backend();
        backend.orchestrator_port = 9999;
        let url = backend.agent_ws_url("test-prefix-abc123");
        assert_eq!(url, Some("ws://host.docker.internal:9999/ws/abc123".to_string()));
    }

    #[test]
    fn agent_ws_url_host_network_custom_port() {
        let mut backend = test_backend();
        backend.network_policy = NetworkPolicy::HostNetwork;
        backend.orchestrator_port = 8080;
        let url = backend.agent_ws_url("test-prefix-abc123");
        assert_eq!(url, Some("ws://127.0.0.1:8080/ws/abc123".to_string()));
    }

    // -- DockerBackend with_config --

    #[test]
    fn docker_backend_with_orchestrator_port() {
        let backend = test_backend();
        assert_eq!(backend.orchestrator_port(), DEFAULT_ORCHESTRATOR_PORT);
    }
}
