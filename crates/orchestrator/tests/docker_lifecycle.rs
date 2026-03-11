//! Integration tests for Docker-based agent lifecycle in the orchestrator.
//!
//! These tests exercise the full agent lifecycle through the Docker execution
//! backend: create → start → verify running → terminate → verify cleanup.
//!
//! # Prerequisites
//!
//! - Running Docker daemon
//! - `alpine:3.19` image available (auto-pulled by Docker)
//!
//! These tests are gated behind `#[ignore]` and must be run explicitly:
//!
//! ```bash
//! cargo test -p orchestrator -- --ignored
//! ```

use wrap::backend::{ExecutionBackend, SessionConfig, SessionHealth};
use wrap::docker::DockerBackend;

const TEST_PREFIX: &str = "agentd-orch-test";
const TEST_IMAGE: &str = "alpine:3.19";

fn test_backend() -> DockerBackend {
    DockerBackend::new(TEST_PREFIX, TEST_IMAGE).expect("Docker client should initialize")
}

fn test_config(suffix: &str) -> SessionConfig {
    SessionConfig {
        session_name: format!("{}-{}", TEST_PREFIX, suffix),
        working_dir: "/tmp".into(),
        agent_type: "general".into(),
        model_provider: "none".into(),
        model_name: "none".into(),
        layout: None,
        network_policy: None,
    }
}

async fn cleanup(backend: &DockerBackend, session_name: &str) {
    let _ = backend.kill_session(session_name).await;
}

// ---------------------------------------------------------------------------
// Full lifecycle: create → start → running → kill → gone
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn full_agent_lifecycle() {
    let backend = test_backend();
    let config = test_config("lifecycle");
    cleanup(&backend, &config.session_name).await;

    // 1. Create the container.
    backend.create_session(&config).await.expect("create should succeed");

    // Verify it appears in the list (created but not started).
    let sessions = backend.list_sessions().await.unwrap();
    assert!(sessions.contains(&config.session_name), "Container should appear after create");

    // 2. Start the container.
    backend.launch_agent(&config).await.expect("launch should succeed");

    // 3. Verify it's running.
    let exists = backend.session_exists(&config.session_name).await.unwrap();
    assert!(exists, "Container should exist after launch");

    // Give it a moment to fully start.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let health = backend.session_health(&config.session_name).await.unwrap();
    assert_eq!(health, SessionHealth::Healthy, "Running container should be healthy");

    // Exit info should be None while running.
    let exit_info = backend.session_exit_info(&config.session_name).await.unwrap();
    assert!(exit_info.is_none(), "Running container should have no exit info");

    // 4. Terminate the container.
    backend.kill_session(&config.session_name).await.expect("kill should succeed");

    // 5. Verify it's gone.
    let exists_after = backend.session_exists(&config.session_name).await.unwrap();
    assert!(!exists_after, "Container should not exist after kill");

    let sessions_after = backend.list_sessions().await.unwrap();
    assert!(
        !sessions_after.contains(&config.session_name),
        "Container should not appear in list after kill"
    );
}

// ---------------------------------------------------------------------------
// Reconciliation with stale containers
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn reconciliation_stale_containers() {
    let backend = test_backend();
    let configs: Vec<_> = (0..3).map(|i| test_config(&format!("reconcile-{}", i))).collect();

    for c in &configs {
        cleanup(&backend, &c.session_name).await;
    }

    // Create and start containers to simulate "stale" sessions.
    for c in &configs {
        backend.create_session(c).await.unwrap();
        backend.launch_agent(c).await.unwrap();
    }

    // Verify all are running.
    for c in &configs {
        let exists = backend.session_exists(&c.session_name).await.unwrap();
        assert!(exists, "{} should be running", c.session_name);
    }

    // Simulate reconciliation: the orchestrator discovers these containers
    // and decides to clean them up (e.g., they have no matching DB record).
    for c in &configs {
        backend.kill_session(&c.session_name).await.unwrap();
    }

    // All should be gone.
    let sessions = backend.list_sessions().await.unwrap();
    for c in &configs {
        assert!(!sessions.contains(&c.session_name), "{} should be cleaned up", c.session_name);
    }
}

// ---------------------------------------------------------------------------
// Multiple backends don't interfere
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn separate_backends_isolate_sessions() {
    let backend_a = DockerBackend::new("agentd-test-a", TEST_IMAGE).unwrap();
    let backend_b = DockerBackend::new("agentd-test-b", TEST_IMAGE).unwrap();

    let config_a = SessionConfig {
        session_name: "agentd-test-a-iso".into(),
        working_dir: "/tmp".into(),
        agent_type: "general".into(),
        model_provider: "none".into(),
        model_name: "none".into(),
        layout: None,
        network_policy: None,
    };
    let config_b = SessionConfig {
        session_name: "agentd-test-b-iso".into(),
        working_dir: "/tmp".into(),
        agent_type: "general".into(),
        model_provider: "none".into(),
        model_name: "none".into(),
        layout: None,
        network_policy: None,
    };

    // Clean up.
    cleanup(&backend_a, &config_a.session_name).await;
    cleanup(&backend_b, &config_b.session_name).await;

    // Create sessions on both backends.
    backend_a.create_session(&config_a).await.unwrap();
    backend_b.create_session(&config_b).await.unwrap();

    // Each backend should only see its own session.
    let sessions_a = backend_a.list_sessions().await.unwrap();
    let sessions_b = backend_b.list_sessions().await.unwrap();

    assert!(sessions_a.contains(&config_a.session_name));
    assert!(!sessions_a.contains(&config_b.session_name));

    assert!(sessions_b.contains(&config_b.session_name));
    assert!(!sessions_b.contains(&config_a.session_name));

    // Clean up.
    cleanup(&backend_a, &config_a.session_name).await;
    cleanup(&backend_b, &config_b.session_name).await;
}
