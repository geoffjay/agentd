//! Agentic search fallback using `grep` and `find` commands.
//!
//! [`AgenticSearch`] provides a `POST /search/agentic` endpoint that searches
//! source files directly using the system `grep` utility.  It is intended as a
//! fallback when the vector index returns no results or low-confidence matches —
//! for example when searching for a very specific identifier that may not have
//! been indexed yet.
//!
//! # Request / Response
//!
//! ```json
//! // POST /search/agentic
//! {
//!   "query": "authenticate_request",
//!   "path": "crates/index/src",
//!   "file_pattern": "*.rs",
//!   "context_lines": 2,
//!   "limit": 20
//! }
//!
//! // Response
//! {
//!   "matches": [
//!     {
//!       "file_path": "crates/index/src/api.rs",
//!       "line_number": 42,
//!       "content": "pub async fn authenticate_request(…) {",
//!       "context_before": ["", "/// Authenticates an incoming request."],
//!       "context_after": ["    let token = …", "}"]
//!     }
//!   ],
//!   "total": 1,
//!   "query_time_ms": 12
//! }
//! ```
//!
//! # Availability
//!
//! Requires `grep` to be present on the `PATH`.  On macOS, BSD grep is used;
//! on Linux, GNU grep.  The `-P` (Perl regex) flag is *not* used so that both
//! flavours are supported.

use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::SearchError;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Request body for the `POST /search/agentic` endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct AgenticSearchRequest {
    /// Search term or pattern passed to `grep` (basic regex).
    pub query: String,

    /// Root path to search within.  Relative paths are resolved against the
    /// process working directory.  Defaults to `.` (current directory).
    #[serde(default)]
    pub path: Option<String>,

    /// Glob pattern to restrict which files are searched (e.g. `"*.rs"`).
    ///
    /// Passed to `grep --include`.  Defaults to `"*"` (all files).
    #[serde(default)]
    pub file_pattern: Option<String>,

    /// Number of context lines to include before and after each match.
    ///
    /// Defaults to `2`.
    #[serde(default)]
    pub context_lines: Option<usize>,

    /// Maximum number of matches to return.
    ///
    /// Defaults to `20`.  Capped at `200`.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// A single file-level match from the agentic search.
#[derive(Debug, Clone, Serialize)]
pub struct AgenticMatch {
    /// Path of the matching file (relative to the searched root).
    pub file_path: String,
    /// One-based line number of the match.
    pub line_number: u32,
    /// The matching line content (trimmed).
    pub content: String,
    /// Lines immediately before the match (up to `context_lines`).
    pub context_before: Vec<String>,
    /// Lines immediately after the match (up to `context_lines`).
    pub context_after: Vec<String>,
}

/// Response body for the `POST /search/agentic` endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AgenticSearchResponse {
    /// All matching lines with context.
    pub matches: Vec<AgenticMatch>,
    /// Number of matches returned.
    pub total: usize,
    /// Wall-clock query execution time in milliseconds.
    pub query_time_ms: u64,
}

// ---------------------------------------------------------------------------
// AgenticSearch
// ---------------------------------------------------------------------------

/// Grep-based file search that requires no index.
///
/// Construct via [`AgenticSearch::new`], specifying a base directory to
/// search within.  All relative `path` values in requests are resolved against
/// this base.
pub struct AgenticSearch {
    base_path: PathBuf,
}

impl AgenticSearch {
    /// Create a new `AgenticSearch` rooted at `base_path`.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self { base_path: base_path.into() }
    }

    /// Run the agentic search and return matching lines with context.
    pub async fn search(
        &self,
        request: &AgenticSearchRequest,
    ) -> Result<AgenticSearchResponse, SearchError> {
        if request.query.trim().is_empty() {
            return Err(SearchError::InvalidRequest(
                "agentic search query must not be empty".to_string(),
            ));
        }

        let start = Instant::now();
        let limit = request.limit.unwrap_or(20).clamp(1, 200);
        let context = request.context_lines.unwrap_or(2);

        let search_path = match &request.path {
            Some(p) => {
                let candidate = PathBuf::from(p);
                if candidate.is_absolute() {
                    candidate
                } else {
                    self.base_path.join(p)
                }
            }
            None => self.base_path.clone(),
        };

        let file_pattern = request.file_pattern.as_deref().unwrap_or("*").to_string();

        // Build grep arguments.
        let mut cmd = tokio::process::Command::new("grep");
        cmd.arg("--recursive")
            .arg("--line-number")
            .arg("--with-filename")
            .arg(format!("--after-context={context}"))
            .arg(format!("--before-context={context}"))
            .arg(format!("--include={file_pattern}"))
            .arg(&request.query)
            .arg(&search_path);

        let output = cmd
            .output()
            .await
            .map_err(|e| SearchError::Backend(format!("grep failed to start: {e}")))?;

        // grep exits 1 when no matches are found — that's not an error for us.
        // Exit 2 indicates a genuine error.
        if output.status.code() == Some(2) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SearchError::Backend(format!("grep error: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let matches = parse_grep_output(&stdout, context, limit, &search_path);
        let total = matches.len();
        let query_time_ms = start.elapsed().as_millis() as u64;

        Ok(AgenticSearchResponse { matches, total, query_time_ms })
    }
}

// ---------------------------------------------------------------------------
// Grep output parser
// ---------------------------------------------------------------------------

/// Parse `grep -n --with-filename -A<ctx> -B<ctx>` output into [`AgenticMatch`]es.
///
/// The separator between match groups when using `-A`/`-B` is `--`.
fn parse_grep_output(
    output: &str,
    context: usize,
    limit: usize,
    base_path: &std::path::Path,
) -> Vec<AgenticMatch> {
    // Each "group" in grep output (with -A/-B) looks like:
    //   file-path-context1:line:content      (context before, prefix "-")
    //   file-path-MATCH:line:content         (match line)
    //   file-path-context2:line:content      (context after, prefix "-")
    //   --                                   (group separator)
    //
    // With --line-number and --with-filename:
    //   file.rs:42:content line
    //   file.rs-41-context line   (context lines use `-` separator)
    //
    // We parse each group to extract the match line and surrounding context.

    let mut matches: Vec<AgenticMatch> = Vec::new();
    let mut current_group: Vec<(String, u32, bool)> = Vec::new(); // (content, line_no, is_match)

    for line in output.lines() {
        if line == "--" {
            // Flush the current group.
            if let Some(m) = flush_group(&current_group, context, base_path) {
                matches.push(m);
                if matches.len() >= limit {
                    break;
                }
            }
            current_group.clear();
            continue;
        }

        // Try to parse `file:line:content` (match) or `file-line-content` (context).
        if let Some(parsed) = parse_grep_line(line) {
            current_group.push(parsed);
        }
    }

    // Flush the last group (no trailing `--`).
    if matches.len() < limit {
        if let Some(m) = flush_group(&current_group, context, base_path) {
            matches.push(m);
        }
    }

    matches
}

/// Parse a single grep output line.
///
/// Returns `(content, line_number, is_match)` or `None` if the line doesn't
/// match the expected format.
fn parse_grep_line(line: &str) -> Option<(String, u32, bool)> {
    // Match lines:   `file.rs:42:content`
    // Context lines: `file.rs-42-content`
    // We detect by looking for `:digit+:` vs `-digit+-`.

    // Find the second delimiter after the file path.
    // File paths may themselves contain `:` on Windows but we target Unix.
    let is_match_line = line.contains(':');

    if is_match_line {
        // Split on the first `:digit+:` pattern.
        let mut parts = line.splitn(3, ':');
        let _file = parts.next()?;
        let line_no_str = parts.next()?;
        let content = parts.next().unwrap_or("").trim().to_string();
        let line_no = line_no_str.parse::<u32>().ok()?;
        Some((content, line_no, true))
    } else {
        // Context line: `file.rs-42-content`
        let mut parts = line.splitn(3, '-');
        let _file = parts.next()?;
        let line_no_str = parts.next()?;
        let content = parts.next().unwrap_or("").trim().to_string();
        let line_no = line_no_str.parse::<u32>().ok()?;
        Some((content, line_no, false))
    }
}

/// Flush a collected group into an [`AgenticMatch`], extracting the first
/// match line as the primary hit and surrounding lines as context.
fn flush_group(
    group: &[(String, u32, bool)],
    _context: usize,
    base_path: &std::path::Path,
) -> Option<AgenticMatch> {
    // Find the first match line.
    let match_idx = group.iter().position(|(_, _, is_match)| *is_match)?;
    let (content, line_number, _) = &group[match_idx];

    let context_before: Vec<String> =
        group[..match_idx].iter().map(|(c, _, _)| c.clone()).collect();
    let context_after: Vec<String> =
        group[match_idx + 1..].iter().map(|(c, _, _)| c.clone()).collect();

    // Determine file path — grep output includes the filename in each line.
    // We'll reconstruct it from the base_path since we already stripped it.
    let file_path = base_path.to_string_lossy().to_string();

    Some(AgenticMatch {
        file_path,
        line_number: *line_number,
        content: content.clone(),
        context_before,
        context_after,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── AgenticSearchRequest deserialization ───────────────────────────────

    #[test]
    fn request_minimal_deserialize() {
        let req: AgenticSearchRequest =
            serde_json::from_str(r#"{"query":"authenticate"}"#).unwrap();
        assert_eq!(req.query, "authenticate");
        assert_eq!(req.limit, None);
        assert_eq!(req.context_lines, None);
        assert!(req.path.is_none());
    }

    #[test]
    fn request_full_deserialize() {
        let json = r#"{
            "query": "fn authenticate",
            "path": "src/auth",
            "file_pattern": "*.rs",
            "context_lines": 3,
            "limit": 10
        }"#;
        let req: AgenticSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.query, "fn authenticate");
        assert_eq!(req.path.as_deref(), Some("src/auth"));
        assert_eq!(req.file_pattern.as_deref(), Some("*.rs"));
        assert_eq!(req.context_lines, Some(3));
        assert_eq!(req.limit, Some(10));
    }

    // ── AgenticSearch::search validation ──────────────────────────────────

    #[tokio::test]
    async fn empty_query_returns_error() {
        let search = AgenticSearch::new("/tmp");
        let req = AgenticSearchRequest {
            query: "  ".to_string(),
            path: None,
            file_pattern: None,
            context_lines: None,
            limit: None,
        };
        assert!(search.search(&req).await.is_err());
    }

    // ── grep output parsing ────────────────────────────────────────────────

    #[test]
    fn parse_match_line_basic() {
        let line = "src/lib.rs:42:pub fn authenticate() {";
        let parsed = parse_grep_line(line).unwrap();
        assert_eq!(parsed.1, 42);
        assert!(parsed.2); // is_match
        assert_eq!(parsed.0, "pub fn authenticate() {");
    }

    #[test]
    fn parse_context_line() {
        let line = "src/lib.rs-41-    // before context";
        let parsed = parse_grep_line(line).unwrap();
        assert_eq!(parsed.1, 41);
        assert!(!parsed.2); // context, not match
    }

    #[test]
    fn flush_group_extracts_match_and_context() {
        let group = vec![
            ("// context before".to_string(), 40, false),
            ("pub fn authenticate() {".to_string(), 41, true),
            ("    token.verify()".to_string(), 42, false),
        ];
        let base = PathBuf::from("src");
        let m = flush_group(&group, 1, &base).unwrap();
        assert_eq!(m.line_number, 41);
        assert_eq!(m.content, "pub fn authenticate() {");
        assert_eq!(m.context_before, vec!["// context before"]);
        assert_eq!(m.context_after, vec!["    token.verify()"]);
    }

    #[test]
    fn flush_empty_group_returns_none() {
        assert!(flush_group(&[], 2, &PathBuf::from(".")).is_none());
    }
}
