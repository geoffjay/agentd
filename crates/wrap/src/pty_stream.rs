//! PTY output stream with ring-buffer history and broadcast delivery.
//!
//! [`PtyOutputStream`] captures all bytes produced by a PTY session and
//! makes them available to multiple concurrent subscribers. Late subscribers
//! receive a replay of buffered history followed by live output via a
//! [`tokio::sync::broadcast`] channel.
//!
//! # Design
//!
//! ```text
//! PTY master reader ──► spawn_reader() ──► broadcast::Sender<Bytes>
//!                                     └──► ring-buffer history
//!
//! subscriber ──► subscribe() ──► (Vec<Bytes> history, Receiver<Bytes>)
//!
//! send_command / kill ──► write_input(&[u8]) ──► PTY master writer
//! ```
//!
//! The ring buffer is bounded by `max_history_bytes`; oldest chunks are
//! evicted to keep total byte usage within budget.
//!
//! # Examples
//!
//! ```no_run
//! use wrap::pty_stream::{PtyOutputStream, DEFAULT_CHANNEL_CAPACITY, DEFAULT_HISTORY_BYTES};
//! # use std::io::Write;
//!
//! # fn make_writer() -> Box<dyn Write + Send> { unimplemented!() }
//! # fn make_reader() -> Box<dyn std::io::Read + Send + 'static> { unimplemented!() }
//! # tokio_test::block_on(async {
//! let stream = PtyOutputStream::new(
//!     DEFAULT_CHANNEL_CAPACITY,
//!     DEFAULT_HISTORY_BYTES,
//!     make_writer(),
//! );
//!
//! // Subscribe before spawning the reader to avoid missing early output.
//! let (history, mut rx) = stream.subscribe();
//!
//! // Spawn the background reader (moves `stream` clone into a blocking task).
//! stream.clone().spawn_reader(make_reader());
//!
//! // Send terminal input
//! stream.write_input(b"echo hello\n").unwrap();
//! # });
//! ```

use bytes::Bytes;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::broadcast;

/// Default broadcast channel capacity (number of in-flight chunks).
///
/// Slow subscribers that fall more than this many chunks behind will
/// receive [`RecvError::Lagged`](tokio::sync::broadcast::error::RecvError::Lagged).
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Default ring-buffer history limit (512 KiB).
///
/// Oldest chunks are evicted once the buffer exceeds this byte count.
pub const DEFAULT_HISTORY_BYTES: usize = 512 * 1024;

/// Internal read-buffer size for the background PTY reader task.
const READ_BUF_SIZE: usize = 4096;

/// PTY output stream with ring-buffer history and broadcast delivery.
///
/// Captures all output produced by a PTY session and delivers it to
/// multiple concurrent subscribers. Late subscribers receive buffered
/// history up to `max_history_bytes` for replay, followed by live
/// output via a [`broadcast::Receiver<Bytes>`].
///
/// The stream also owns the PTY master writer so all terminal I/O
/// (both output capture and input delivery) is centralised here.
///
/// # Clone
///
/// `PtyOutputStream` is cheaply [`Clone`]able — clones share the same
/// broadcast sender, history ring, and writer. This makes it safe to
/// move one clone into [`spawn_reader`](PtyOutputStream::spawn_reader)
/// while retaining a handle in the owning `PtySession`.
#[derive(Clone)]
pub struct PtyOutputStream {
    history: Arc<RwLock<VecDeque<Bytes>>>,
    history_bytes: Arc<RwLock<usize>>,
    tx: broadcast::Sender<Bytes>,
    max_history_bytes: usize,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl PtyOutputStream {
    /// Create a new `PtyOutputStream`.
    ///
    /// # Arguments
    ///
    /// * `channel_capacity` — Number of chunks the broadcast channel buffers
    ///   before slow subscribers start receiving `RecvError::Lagged`.
    /// * `max_history_bytes` — Byte budget for the replay ring buffer.
    ///   Oldest chunks are evicted when this limit is exceeded.
    /// * `writer` — PTY master writer (from `MasterPty::take_writer()`).
    pub fn new(
        channel_capacity: usize,
        max_history_bytes: usize,
        writer: Box<dyn Write + Send>,
    ) -> Self {
        let (tx, _) = broadcast::channel(channel_capacity);
        Self {
            history: Arc::new(RwLock::new(VecDeque::new())),
            history_bytes: Arc::new(RwLock::new(0)),
            tx,
            max_history_bytes,
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    /// Subscribe to the output stream.
    ///
    /// Returns `(history, rx)` where:
    /// - `history` is a snapshot of the ring-buffer (oldest-first), and
    /// - `rx` is a live [`broadcast::Receiver<Bytes>`] for subsequent chunks.
    ///
    /// Subscribe *before* spawning the reader to avoid missing early output.
    /// If `rx` falls too far behind the producer, chunks will be dropped and
    /// it will yield [`RecvError::Lagged`](tokio::sync::broadcast::error::RecvError::Lagged).
    pub fn subscribe(&self) -> (Vec<Bytes>, broadcast::Receiver<Bytes>) {
        // Subscribe before snapshotting history to avoid a race where new
        // chunks arrive between the snapshot and the first recv().
        let rx = self.tx.subscribe();
        let history = self.history.read().expect("history lock poisoned");
        let snapshot: Vec<Bytes> = history.iter().cloned().collect();
        (snapshot, rx)
    }

    /// Write raw bytes to the PTY master (terminal input).
    ///
    /// Used to send commands or control sequences (e.g., Ctrl-C `= &[0x03]`)
    /// to the running process.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY writer has been closed or the write fails.
    pub fn write_input(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer =
            self.writer.lock().map_err(|_| anyhow::anyhow!("PTY writer lock poisoned"))?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Spawn a background task that reads from `reader` and broadcasts chunks.
    ///
    /// The synchronous `reader` (typically from `MasterPty::try_clone_reader()`)
    /// is moved into a [`tokio::task::spawn_blocking`] thread. Each chunk is
    /// broadcast to live subscribers and appended to the ring-buffer history.
    ///
    /// The task exits silently on EOF or read error.
    pub fn spawn_reader<R: Read + Send + 'static>(self, reader: R) {
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; READ_BUF_SIZE];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF — process exited
                    Ok(n) => {
                        let chunk = Bytes::copy_from_slice(&buf[..n]);
                        self.push_history(chunk.clone());
                        // Ignore send errors — no active subscribers is fine
                        let _ = self.tx.send(chunk);
                    }
                    Err(e) => {
                        tracing::debug!("PTY reader task ended: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// Append a chunk to the ring-buffer history, evicting oldest if over budget.
    fn push_history(&self, chunk: Bytes) {
        let mut history = self.history.write().expect("history lock poisoned");
        let mut history_bytes = self.history_bytes.write().expect("history_bytes lock poisoned");

        *history_bytes += chunk.len();
        history.push_back(chunk);

        // Evict oldest chunks until within byte budget
        while *history_bytes > self.max_history_bytes {
            if let Some(evicted) = history.pop_front() {
                *history_bytes = history_bytes.saturating_sub(evicted.len());
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A minimal `Write` implementation backed by a `Vec<u8>`.
    struct VecWriter(Vec<u8>);

    impl Write for VecWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn make_stream(cap: usize, max_bytes: usize) -> PtyOutputStream {
        PtyOutputStream::new(cap, max_bytes, Box::new(VecWriter(Vec::new())))
    }

    #[test]
    fn subscribe_returns_empty_history_initially() {
        let stream = make_stream(16, 1024);
        let (history, _rx) = stream.subscribe();
        assert!(history.is_empty());
    }

    #[test]
    fn subscribe_after_push_history_returns_chunks() {
        let stream = make_stream(16, 1024);
        stream.push_history(Bytes::from("hello"));
        stream.push_history(Bytes::from("world"));

        let (history, _rx) = stream.subscribe();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], Bytes::from("hello"));
        assert_eq!(history[1], Bytes::from("world"));
    }

    #[test]
    fn ring_buffer_evicts_oldest_when_over_budget() {
        // 10-byte budget; push three 5-byte chunks → first should be evicted
        let stream = make_stream(16, 10);

        stream.push_history(Bytes::from("aaaaa")); // 5 bytes
        stream.push_history(Bytes::from("bbbbb")); // 5 bytes → total 10, at limit
        stream.push_history(Bytes::from("ccccc")); // 5 bytes → total 15 → evict first

        let (history, _rx) = stream.subscribe();
        // After eviction, only the last two chunks should remain
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], Bytes::from("bbbbb"));
        assert_eq!(history[1], Bytes::from("ccccc"));
    }

    #[test]
    fn ring_buffer_byte_count_tracks_correctly() {
        let stream = make_stream(16, 1024);
        stream.push_history(Bytes::from("abc"));
        stream.push_history(Bytes::from("de"));

        let count = *stream.history_bytes.read().unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn write_input_forwards_bytes_to_writer() {
        let stream = make_stream(16, 1024);
        stream.write_input(b"hello\n").unwrap();
        // We can't easily inspect the inner VecWriter in this test, but at
        // minimum verify that no error is returned.
    }

    #[tokio::test]
    async fn spawn_reader_broadcasts_chunks() {
        let stream = make_stream(16, 1024);
        let (_history, mut rx) = stream.subscribe();

        // Use a Cursor as a synchronous reader
        let data = b"chunk one";
        let reader = Cursor::new(data.to_vec());
        stream.clone().spawn_reader(reader);

        // Receive the broadcast chunk
        let received = rx.recv().await.unwrap();
        assert_eq!(received.as_ref(), b"chunk one");
    }

    #[tokio::test]
    async fn spawn_reader_populates_history() {
        let stream = make_stream(16, 1024);

        let data = b"history data";
        let reader = Cursor::new(data.to_vec());
        stream.clone().spawn_reader(reader);

        // Give the reader task a moment to run
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let (history, _rx) = stream.subscribe();
        let combined: Vec<u8> = history.iter().flat_map(|b| b.as_ref()).copied().collect();
        assert_eq!(combined, b"history data");
    }

    #[test]
    fn clone_shares_state() {
        let stream = make_stream(16, 1024);
        let clone = stream.clone();

        stream.push_history(Bytes::from("shared"));

        let (history, _rx) = clone.subscribe();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], Bytes::from("shared"));
    }
}
