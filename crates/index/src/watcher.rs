//! File-system watcher for automatic re-indexing.
//!
//! [`FileWatcher`] monitors one or more repository root directories using the
//! platform-native OS notification mechanism (via the [`notify`] crate) and
//! forwards changed file paths to a [`tokio::sync::mpsc`] channel with a
//! configurable debounce window.
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//! use index::watcher::FileWatcher;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
//!
//! let _watcher = FileWatcher::watch(
//!     &[PathBuf::from("/home/user/my-project")],
//!     500,   // debounce_ms
//!     tx,
//! )?;
//!
//! while let Some(changed) = rx.recv().await {
//!     println!("Changed: {}", changed.display());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Debouncing
//!
//! File-system events often arrive in rapid bursts (e.g. when an editor saves
//! multiple files or a build tool generates artefacts).  The watcher suppresses
//! duplicate paths within the debounce window: a path is emitted at most once
//! per window regardless of how many events are received for it.
//!
//! The debounce is implemented in a background tokio task that collects events
//! arriving within `debounce_ms` of the first event in a batch, then flushes
//! the deduplicated set.
//!
//! # Supported events
//!
//! Only [`notify::EventKind::Create`] and [`notify::EventKind::Modify`] events
//! are forwarded.  Remove / rename / access events are ignored.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// FileWatcher
// ---------------------------------------------------------------------------

/// Watches directory trees for file changes and emits paths to a channel.
///
/// The watcher must be kept alive for as long as watching is desired.  Dropping
/// it stops the OS-level watch.
pub struct FileWatcher {
    /// Inner notify watcher — must be kept alive.
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Start watching `paths` for file-system changes.
    ///
    /// Changed file paths are sent to `tx`.  Paths are deduplicated within a
    /// `debounce_ms` window before being forwarded.
    ///
    /// # Errors
    ///
    /// Returns a [`notify::Error`] if the OS watcher cannot be created or if
    /// any of the provided paths cannot be watched (e.g. they do not exist).
    pub fn watch(
        paths: &[PathBuf],
        debounce_ms: u64,
        tx: UnboundedSender<PathBuf>,
    ) -> Result<Self, notify::Error> {
        // Bridge: notify events arrive on a sync mpsc; we bridge to tokio.
        let (sync_tx, sync_rx) = std::sync::mpsc::channel::<Event>();

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    let _ = sync_tx.send(event);
                }
                Err(e) => {
                    warn!("notify watcher error: {e}");
                }
            })?;

        for path in paths {
            watcher.watch(path, RecursiveMode::Recursive)?;
            debug!(path = %path.display(), "Watching path for changes");
        }

        // Spawn a blocking thread to receive events from the sync channel and
        // forward them (with debouncing) to the async sender.
        let debounce = Duration::from_millis(debounce_ms);
        std::thread::spawn(move || {
            debounce_thread(sync_rx, tx, debounce);
        });

        Ok(Self { _watcher: watcher })
    }
}

// ---------------------------------------------------------------------------
// Debounce thread
// ---------------------------------------------------------------------------

/// Receives raw [`notify::Event`]s from `rx`, debounces within `window`, and
/// forwards unique changed paths to `tx`.
fn debounce_thread(
    rx: std::sync::mpsc::Receiver<Event>,
    tx: UnboundedSender<PathBuf>,
    window: Duration,
) {
    while let Ok(first) = rx.recv() {
        // sender dropped when Err — loop ends naturally

        let mut pending: HashSet<PathBuf> = collect_paths(first);

        // Drain any additional events that arrive within the debounce window.
        let deadline = std::time::Instant::now() + window;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match rx.recv_timeout(remaining) {
                Ok(event) => {
                    pending.extend(collect_paths(event));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Flush remaining then exit.
                    for path in pending {
                        let _ = tx.send(path);
                    }
                    return;
                }
            }
        }

        // Forward all deduplicated paths.
        for path in pending {
            debug!(path = %path.display(), "File change detected");
            if tx.send(path).is_err() {
                return; // receiver dropped
            }
        }
    }
}

/// Extract the file paths from a [`notify::Event`], filtering to only
/// create/modify events.
fn collect_paths(event: Event) -> HashSet<PathBuf> {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => event.paths.into_iter().collect(),
        _ => HashSet::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_paths_create_returns_paths() {
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("/tmp/foo.rs"), PathBuf::from("/tmp/bar.rs")],
            attrs: Default::default(),
        };
        let paths = collect_paths(event);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/tmp/foo.rs")));
    }

    #[test]
    fn collect_paths_modify_returns_paths() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![PathBuf::from("/tmp/main.rs")],
            attrs: Default::default(),
        };
        let paths = collect_paths(event);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn collect_paths_remove_returns_empty() {
        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![PathBuf::from("/tmp/gone.rs")],
            attrs: Default::default(),
        };
        assert!(collect_paths(event).is_empty());
    }

    #[test]
    fn collect_paths_access_returns_empty() {
        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Read),
            paths: vec![PathBuf::from("/tmp/read.rs")],
            attrs: Default::default(),
        };
        assert!(collect_paths(event).is_empty());
    }

    #[tokio::test]
    async fn watch_nonexistent_path_returns_error() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
        let result = FileWatcher::watch(&[PathBuf::from("/nonexistent/path/abc")], 100, tx);
        assert!(result.is_err(), "Watching a non-existent path should fail");
    }

    #[tokio::test]
    async fn watch_existing_path_starts_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
        let result = FileWatcher::watch(&[dir.path().to_path_buf()], 100, tx);
        assert!(result.is_ok(), "Watching an existing directory should succeed");
    }
}
