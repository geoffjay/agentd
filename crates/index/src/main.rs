//! agentd-index service entry point.
//!
//! Initialises the HTTP server with health, metrics, search, and repository
//! management endpoints.  Also starts a background file-watcher that
//! automatically re-indexes repositories when source files change.
//!
//! # Running the Service
//!
//! ```bash
//! cargo run -p index
//! RUST_LOG=debug cargo run -p index
//! ```
//!
//! # Environment Variables
//!
//! | Variable                        | Default                        | Description               |
//! |---------------------------------|--------------------------------|---------------------------|
//! | `RUST_LOG`                      | `info`                         | Log level                 |
//! | `AGENTD_PORT`                   | `17012`                        | HTTP listen port          |
//! | `AGENTD_INDEX_LANCE_PATH`       | XDG data dir / `lancedb`       | LanceDB directory         |
//! | `AGENTD_INDEX_EMBEDDING_MODEL`  | `nomic-embed-text`             | Embedding model           |
//!
//! # Endpoints
//!
//! - `GET /health`                    — health check
//! - `GET /metrics`                   — Prometheus metrics
//! - `POST /search`                   — semantic vector search over code chunks
//! - `POST /search/agentic`           — grep-based fallback search
//! - `POST /repositories`             — register a repository
//! - `GET  /repositories`             — list repositories
//! - `GET  /repositories/:id`         — get repository
//! - `DELETE /repositories/:id`       — remove repository
//! - `GET  /repositories/:id/status`  — repository indexing status
//! - `POST /repositories/:id/reindex` — trigger re-indexing

use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, response::IntoResponse, routing::get};
use index::api::{create_router_with_state, AppState};
use index::config::IndexConfig;
use index::indexer::{Indexer, IndexerConfig};
use index::repository::{RepoStatus, RepoStore};
use index::store::{create_store, CodeStore};
use index::watcher::FileWatcher;
use metrics_exporter_prometheus::PrometheusHandle;
use tracing::{info, warn};

/// Raise the open-file-descriptor soft limit to the process hard limit (capped
/// at 65 536).  Lance memory-maps index partitions and data fragment files for
/// every vector search; the macOS default of 256 fds is far too low and causes
/// `LanceError(IO): Too many open files (os error 24)` under normal load.
#[cfg(unix)]
fn raise_fd_limit() {
    unsafe {
        let mut rl = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            let target = rl.rlim_max.min(65_536);
            if rl.rlim_cur < target {
                rl.rlim_cur = target;
                if libc::setrlimit(libc::RLIMIT_NOFILE, &rl) == 0 {
                    // tracing not yet initialised here; eprintln is fine.
                    eprintln!("agentd-index: raised RLIMIT_NOFILE to {target}");
                } else {
                    eprintln!(
                        "agentd-index: could not raise RLIMIT_NOFILE to {target}: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
    }
}

fn init_metrics() -> PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    let handle = builder.install_recorder().expect("failed to install metrics recorder");
    metrics::gauge!("service_info", "version" => env!("CARGO_PKG_VERSION"), "service" => "index")
        .set(1.0);
    handle
}

async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    handle.render()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(unix)]
    raise_fd_limit();

    agentd_common::server::init_tracing();

    info!("Starting agentd-index service...");

    let config = IndexConfig::load();

    // ── LanceDB directory ─────────────────────────────────────────────────
    std::fs::create_dir_all(&config.lance.path)?;
    info!(lance_path = %config.lance.path, "LanceDB directory ready");

    // ── Vector store ──────────────────────────────────────────────────────
    let store = create_store(&config.lance, &config.embedding).await?;
    store.initialize().await?;
    info!("Vector store initialised");

    // ── Repository store ──────────────────────────────────────────────────
    // Stored next to the LanceDB directory as `repos.json`.
    let repos_file = PathBuf::from(&config.lance.path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("repos.json");

    let repo_store: Arc<RepoStore> = match RepoStore::load(&repos_file).await {
        Ok(s) => {
            info!(path = %repos_file.display(), "Repo store loaded");
            Arc::new(s)
        }
        Err(e) => {
            warn!(%e, path = %repos_file.display(), "Failed to load repo store; starting empty");
            RepoStore::new(&repos_file)
        }
    };

    // ── Background watcher + indexing loop ────────────────────────────────
    {
        let store_clone = Arc::clone(&store);
        let repo_store_clone = Arc::clone(&repo_store);
        let indexer_config = IndexerConfig {
            extensions: config.languages.iter().flat_map(|l| lang_to_ext(l)).collect(),
            ignore_dirs: config.ignore_patterns.clone(),
            ..Default::default()
        };
        let debounce_ms = (config.watch.interval_secs * 1000).min(5000);

        tokio::spawn(async move {
            run_watcher_loop(store_clone, repo_store_clone, indexer_config, debounce_ms).await;
        });
    }

    // ── Metrics ───────────────────────────────────────────────────────────
    let metrics_handle = init_metrics();
    let metrics_router =
        axum::Router::new().route("/metrics", get(metrics_handler)).with_state(metrics_handle);

    // ── Router ────────────────────────────────────────────────────────────
    let app = create_router_with_state(AppState { store, repo_store })
        .merge(metrics_router)
        .layer(agentd_common::server::trace_layer())
        .layer(agentd_common::server::cors_layer());

    let host = env::var("AGENTD_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr = format!("{host}:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Index API server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Background watcher loop
// ---------------------------------------------------------------------------

/// Watches all registered repositories and triggers incremental re-indexing
/// when source files change.
///
/// Steps on startup:
/// 1. Collect all repo paths (non-Error status) and start a [`FileWatcher`].
/// 2. Enqueue any `Pending` repos for immediate indexing.
/// 3. For each incoming changed path, find its owning repo and re-index.
async fn run_watcher_loop(
    store: Arc<dyn CodeStore>,
    repo_store: Arc<RepoStore>,
    indexer_config: IndexerConfig,
    debounce_ms: u64,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

    // Collect paths to watch.
    let repos = repo_store.list().await;
    let watch_paths: Vec<PathBuf> = repos
        .iter()
        .filter(|r| r.status != RepoStatus::Error)
        .map(|r| PathBuf::from(&r.path))
        .filter(|p| p.exists())
        .collect();

    let _watcher = if watch_paths.is_empty() {
        info!("No repositories to watch; file watcher idle");
        None
    } else {
        match FileWatcher::watch(&watch_paths, debounce_ms, tx.clone()) {
            Ok(w) => {
                info!(count = watch_paths.len(), "File watcher started");
                Some(w)
            }
            Err(e) => {
                warn!(%e, "Failed to start file watcher; continuing without watch");
                None
            }
        }
    };

    // Kick off indexing for any Pending repos immediately.
    for repo in repos.iter().filter(|r| r.status == RepoStatus::Pending) {
        let _ = tx.send(PathBuf::from(&repo.path));
    }

    // Process file-change events.
    while let Some(changed_path) = rx.recv().await {
        // Find the registered repo that owns this changed path.
        let repos = repo_store.list().await;
        let Some(repo) = repos.iter().find(|r| {
            let repo_path = PathBuf::from(&r.path);
            changed_path.starts_with(&repo_path) || changed_path == repo_path
        }) else {
            continue;
        };

        let repo_id = repo.id.clone();
        let repo_path = PathBuf::from(&repo.path);

        if !repo_path.exists() {
            warn!(repo_id, path = %repo_path.display(), "Repo path not found; skipping");
            continue;
        }

        info!(repo_id, path = %repo_path.display(), "Starting re-index");
        let _ = repo_store.update_status(&repo_id, RepoStatus::Indexing, None).await;

        let indexer = Indexer::new(Arc::clone(&store), indexer_config.clone());
        match indexer.index_repository(&repo_path, &repo_id, None).await {
            Ok((files, chunks)) => {
                info!(repo_id, files, chunks, "Re-index complete");
                let _ = repo_store.set_last_indexed(&repo_id).await;
            }
            Err(e) => {
                warn!(repo_id, %e, "Re-index failed");
                let _ = repo_store
                    .update_status(&repo_id, RepoStatus::Error, Some(e.to_string()))
                    .await;
            }
        }

        // Brief throttle to avoid tight re-index loops on bursts.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a language name to its common file extensions.
fn lang_to_ext(lang: &str) -> Vec<String> {
    match lang {
        "rust" => vec!["rs".to_string()],
        "python" => vec!["py".to_string()],
        "javascript" => vec!["js".to_string(), "jsx".to_string(), "mjs".to_string()],
        "typescript" => vec!["ts".to_string(), "tsx".to_string()],
        other => vec![other.to_string()],
    }
}
