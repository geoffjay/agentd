//! Integration tests for the Docker execution backend.
//!
//! These tests require a running Docker daemon and the `agentd-claude:latest`
//! image to be available locally. They are gated behind `#[ignore]` so they
//! don't run in normal CI — use `cargo test -p wrap -- --ignored` to run them
//! explicitly, or rely on the dedicated Docker integration CI job.
//!
//! # Prerequisites
//!
//! ```bash
//! docker build -t agentd-claude:latest docker/claude-code/
//! ```

use wrap::backend::{ExecutionBackend, SessionConfig, SessionHealth};
use wrap::docker::{DockerBackend, NetworkPolicy, ResourceLimits};

/// Test prefix to avoid collisions with real sessions.
const TEST_PREFIX: &str = "agentd-docker-test";

/// A lightweight image that's quick to pull and runs indefinitely.
/// Used instead of agentd-claude:latest for faster integration tests.
const TEST_IMAGE: &str = "alpine:3.19";

/// Helper to create a `DockerBackend` for integration tests.
fn test_backend() -> DockerBackend {
    DockerBackend::new(TEST_PREFIX, TEST_IMAGE).expect("Docker client should initialize")
}

/// Helper to create a `DockerBackend` with custom resource limits.
fn test_backend_with_limits(memory_bytes: i64, nano_cpus: i64) -> DockerBackend {
    let limits = ResourceLimits { memory_bytes, nano_cpus };
    DockerBackend::with_config(TEST_PREFIX, TEST_IMAGE, NetworkPolicy::Internet, limits)
        .expect("Docker client should initialize")
}

/// Helper to create a `SessionConfig` with a unique session name.
fn test_config(suffix: &str) -> SessionConfig {
    SessionConfig {
        // Use "general" agent type so the container runs /bin/bash (or sleep)
        // instead of requiring the claude CLI.
        session_name: format!("{}-{}", TEST_PREFIX, suffix),
        working_dir: "/tmp".into(),
        agent_type: "general".into(),
        model_provider: "none".into(),
        model_name: "none".into(),
        layout: None,
        network_policy: None,
    }
}

/// Cleanup helper: best-effort kill of a session (ignores errors).
async fn cleanup(backend: &DockerBackend, session_name: &str) {
    let _ = backend.kill_session(session_name).await;
}

// ---------------------------------------------------------------------------
// Container lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_create_and_list_session() {
    let backend = test_backend();
    let config = test_config("create-list");

    // Ensure clean slate.
    cleanup(&backend, &config.session_name).await;

    // Create a container (not started yet).
    backend.create_session(&config).await.expect("create_session should succeed");

    // The container should appear in the list.
    let sessions = backend.list_sessions().await.expect("list_sessions should succeed");
    assert!(
        sessions.contains(&config.session_name),
        "Created container should appear in list_sessions, got: {:?}",
        sessions
    );

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_create_start_and_check_exists() {
    let backend = test_backend();
    let config = test_config("start-exists");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.expect("create_session should succeed");
    backend.launch_agent(&config).await.expect("launch_agent should succeed");

    let exists =
        backend.session_exists(&config.session_name).await.expect("session_exists should succeed");
    assert!(exists, "Running container should exist");

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_kill_session_removes_container() {
    let backend = test_backend();
    let config = test_config("kill-remove");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    // Kill and remove.
    backend.kill_session(&config.session_name).await.expect("kill_session should succeed");

    // Container should no longer exist.
    let exists = backend.session_exists(&config.session_name).await.unwrap();
    assert!(!exists, "Killed container should not exist");

    // Verify it's not in the list either.
    let sessions = backend.list_sessions().await.unwrap();
    assert!(
        !sessions.contains(&config.session_name),
        "Killed container should not appear in list_sessions"
    );
}

#[tokio::test]
#[ignore]
async fn docker_kill_nonexistent_is_idempotent() {
    let backend = test_backend();
    // Killing a container that doesn't exist should not error.
    let result = backend.kill_session("agentd-docker-test-nonexistent-12345").await;
    assert!(result.is_ok(), "Killing a nonexistent container should be idempotent");
}

#[tokio::test]
#[ignore]
async fn docker_create_is_idempotent() {
    let backend = test_backend();
    let config = test_config("idempotent-create");
    cleanup(&backend, &config.session_name).await;

    // Create twice — second call should not error (409 Conflict is handled).
    backend.create_session(&config).await.unwrap();
    let result = backend.create_session(&config).await;
    assert!(result.is_ok(), "Creating a container twice should be idempotent");

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// Container cleanup — no orphans after termination
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_no_orphaned_containers_after_kill() {
    let backend = test_backend();
    let configs: Vec<_> = (0..3).map(|i| test_config(&format!("orphan-{}", i))).collect();

    // Clean up any leftovers.
    for c in &configs {
        cleanup(&backend, &c.session_name).await;
    }

    // Create and start multiple containers.
    for c in &configs {
        backend.create_session(c).await.unwrap();
        backend.launch_agent(c).await.unwrap();
    }

    // Kill all of them.
    for c in &configs {
        backend.kill_session(&c.session_name).await.unwrap();
    }

    // No containers with our prefix should remain.
    let sessions = backend.list_sessions().await.unwrap();
    let remaining: Vec<_> =
        sessions.iter().filter(|s| configs.iter().any(|c| c.session_name == **s)).collect();
    assert!(remaining.is_empty(), "No orphaned containers should remain: {:?}", remaining);
}

// ---------------------------------------------------------------------------
// shutdown_all_sessions
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_shutdown_all_sessions() {
    let backend = test_backend();
    let configs: Vec<_> = (0..2).map(|i| test_config(&format!("shutdown-{}", i))).collect();

    for c in &configs {
        cleanup(&backend, &c.session_name).await;
    }

    for c in &configs {
        backend.create_session(c).await.unwrap();
        backend.launch_agent(c).await.unwrap();
    }

    // Shutdown all.
    backend.shutdown_all_sessions().await.expect("shutdown_all should succeed");

    let sessions = backend.list_sessions().await.unwrap();
    let remaining: Vec<_> =
        sessions.iter().filter(|s| configs.iter().any(|c| c.session_name == **s)).collect();
    assert!(remaining.is_empty(), "All containers should be removed after shutdown_all");
}

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_resource_limits_applied() {
    // Use custom limits: 512 MiB memory, 1 CPU.
    let memory = 512 * 1024 * 1024;
    let cpus = 1_000_000_000_i64;
    let backend = test_backend_with_limits(memory, cpus);
    let config = test_config("resource-limits");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    // Inspect the container to verify limits were applied.
    // We use bollard directly since DockerBackend doesn't expose raw inspect.
    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();

    let host_config = info.host_config.expect("host_config should be present");
    assert_eq!(host_config.memory, Some(memory), "Memory limit should match");
    assert_eq!(host_config.nano_cpus, Some(cpus), "CPU limit should match");

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// Network policy
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_network_policy_internet() {
    let backend = test_backend(); // Default is Internet (bridge).
    let config = test_config("net-internet");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let host_config = info.host_config.expect("host_config should be present");
    assert_eq!(host_config.network_mode, Some("bridge".to_string()));

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_network_policy_isolated() {
    let backend = DockerBackend::with_config(
        TEST_PREFIX,
        TEST_IMAGE,
        NetworkPolicy::Isolated,
        ResourceLimits::default(),
    )
    .unwrap();
    let config = test_config("net-isolated");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let host_config = info.host_config.expect("host_config should be present");
    assert_eq!(host_config.network_mode, Some("bridge".to_string()));
    // Isolated mode should have empty DNS.
    assert_eq!(host_config.dns, Some(vec![]), "Isolated mode should have empty DNS");

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_network_policy_session_override() {
    let backend = test_backend(); // Default: Internet
    let mut config = test_config("net-override");
    config.network_policy = Some(NetworkPolicy::Isolated);
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let host_config = info.host_config.expect("host_config should be present");
    // Per-session override should take effect.
    assert_eq!(host_config.dns, Some(vec![]), "Session-level Isolated override should apply");

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// Volume mounts
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_volume_mount_working_dir() {
    let backend = test_backend();
    let config = test_config("vol-mount");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let host_config = info.host_config.expect("host_config should be present");
    let binds = host_config.binds.expect("binds should be present");
    assert!(
        binds.iter().any(|b| b.contains(":/workspace:rw")),
        "Working directory should be mounted at /workspace, got: {:?}",
        binds
    );

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// Environment variables
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_env_vars_passed() {
    let backend = test_backend();
    let config = test_config("env-vars");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let container_config = info.config.expect("config should be present");
    let env = container_config.env.expect("env should be present");

    // Verify agent metadata env vars are set.
    assert!(env.iter().any(|e| e == "AGENT_TYPE=general"), "AGENT_TYPE should be set");
    assert!(env.iter().any(|e| e == "MODEL_PROVIDER=none"), "MODEL_PROVIDER should be set");
    assert!(env.iter().any(|e| e == "MODEL_NAME=none"), "MODEL_NAME should be set");

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// Container health (requires image with HEALTHCHECK)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_session_health_no_healthcheck() {
    // alpine has no HEALTHCHECK — should report Healthy for a running container.
    let backend = test_backend();
    let config = test_config("health-no-check");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    // Give the container a moment to start.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let health = backend.session_health(&config.session_name).await.unwrap();
    assert_eq!(
        health,
        SessionHealth::Healthy,
        "Running container without HEALTHCHECK should be Healthy"
    );

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_session_health_nonexistent() {
    let backend = test_backend();
    let health = backend.session_health("agentd-docker-test-nonexistent").await.unwrap();
    assert_eq!(health, SessionHealth::Unknown, "Nonexistent container should be Unknown");
}

// ---------------------------------------------------------------------------
// Exit info
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_exit_info_running_returns_none() {
    let backend = test_backend();
    let config = test_config("exit-running");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let exit_info = backend.session_exit_info(&config.session_name).await.unwrap();
    assert!(exit_info.is_none(), "Running container should return None for exit info");

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_exit_info_nonexistent_returns_none() {
    let backend = test_backend();
    let exit_info = backend.session_exit_info("agentd-docker-test-nonexistent").await.unwrap();
    assert!(exit_info.is_none(), "Nonexistent container should return None");
}

// ---------------------------------------------------------------------------
// Container state
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_container_state_running() {
    let backend = test_backend();
    let config = test_config("state-running");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let state = backend.container_state(&config.session_name).await.unwrap();
    assert_eq!(state, Some("running".to_string()), "Started container should be running");

    cleanup(&backend, &config.session_name).await;
}

#[tokio::test]
#[ignore]
async fn docker_container_state_nonexistent() {
    let backend = test_backend();
    let state = backend.container_state("agentd-docker-test-nonexistent").await.unwrap();
    assert!(state.is_none(), "Nonexistent container state should be None");
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_labels_set_correctly() {
    let backend = test_backend();
    let config = test_config("labels-check");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();

    let docker = bollard::Docker::connect_with_local_defaults().unwrap();
    let info = docker.inspect_container(&config.session_name, None).await.unwrap();
    let container_config = info.config.expect("config should be present");
    let labels = container_config.labels.expect("labels should be present");

    assert_eq!(labels.get("agentd.prefix"), Some(&TEST_PREFIX.to_string()));
    assert_eq!(labels.get("agentd.session"), Some(&config.session_name));
    assert_eq!(labels.get("agentd.agent-id"), Some(&"labels-check".to_string()));

    cleanup(&backend, &config.session_name).await;
}

// ---------------------------------------------------------------------------
// send_command (exec)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn docker_send_command_exec() {
    let backend = test_backend();
    let config = test_config("send-cmd");
    cleanup(&backend, &config.session_name).await;

    backend.create_session(&config).await.unwrap();
    backend.launch_agent(&config).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Send a simple command — should not error.
    let result = backend.send_command(&config.session_name, "echo hello").await;
    assert!(result.is_ok(), "send_command should succeed on a running container");

    cleanup(&backend, &config.session_name).await;
}
