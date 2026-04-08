/**
 * TypeScript types for the Index service (port 17012).
 * Mirrors the Rust types in crates/index.
 */

// ---------------------------------------------------------------------------
// Search types
// ---------------------------------------------------------------------------

/** Search strategy to use for code search. */
export type CodeSearchMode = "vector" | "keyword" | "hybrid";

/** Request body for POST /search */
export interface CodeSearchRequest {
	/** Natural language query or identifier. */
	query: string;
	/** Optional repository filter — only return chunks from this repo. */
	repo_id?: string;
	/** Optional language filter e.g. "rust", "python". */
	language?: string;
	/** Optional glob pattern to filter by file path e.g. "src/auth/**". */
	file_pattern?: string;
	/** Optional hierarchy level filter: "symbol" | "file" | "directory" | "repository". */
	hierarchy_level?: string;
	/** Maximum results to return (default 10, clamped [1, 100]). */
	limit?: number;
	/** Search strategy (default "hybrid"). */
	search_mode: CodeSearchMode;
}

/** A single ranked code chunk returned by POST /search. */
export interface CodeSearchResultItem {
	/** Unique chunk identifier (chunk_<hash>_<seq>). */
	id: string;
	/** Source file path of the chunk. */
	file_path: string;
	/** Programming language e.g. "rust". */
	language: string;
	/** Syntactic kind e.g. "function", "struct". */
	chunk_type: string;
	/** Top-level symbol name if present. */
	symbol_name?: string;
	/** One-based start line. */
	start_line: number;
	/** One-based end line (inclusive). */
	end_line: number;
	/** Full chunk source text. */
	content: string;
	/** LLM-generated natural language summary. */
	summary?: string;
	/** Relevance score (higher is better). */
	score: number;
	/** Repository the chunk belongs to. */
	repo_id: string;
}

/** Response body from POST /search. */
export interface CodeSearchResponse {
	/** Ranked list of matching code chunks. */
	results: CodeSearchResultItem[];
	/** Number of results returned. */
	total: number;
	/** Wall-clock query time in milliseconds. */
	query_time_ms: number;
}

// ---------------------------------------------------------------------------
// Agentic (grep-based) search types
// ---------------------------------------------------------------------------

/** Request body for POST /search/agentic */
export interface CodeAgenticSearchRequest {
	/** Search term or basic regex pattern. */
	query: string;
	/** Root path to search within (default "."). */
	path?: string;
	/** Glob pattern to restrict files e.g. "*.rs". */
	file_pattern?: string;
	/** Context lines before/after each match (default 2). */
	context_lines?: number;
	/** Maximum matches to return (default 20, capped at 200). */
	limit?: number;
}

/** A single grep match from POST /search/agentic. */
export interface CodeAgenticMatch {
	/** Path of the matching file. */
	file_path: string;
	/** One-based line number of the match. */
	line_number: number;
	/** The matching line content (trimmed). */
	content: string;
	/** Lines immediately before the match. */
	context_before: string[];
	/** Lines immediately after the match. */
	context_after: string[];
}

/** Response body from POST /search/agentic. */
export interface CodeAgenticSearchResponse {
	/** All matching lines with context. */
	matches: CodeAgenticMatch[];
	/** Number of matches returned. */
	total: number;
	/** Wall-clock query time in milliseconds. */
	query_time_ms: number;
}

// ---------------------------------------------------------------------------
// Repository types
// ---------------------------------------------------------------------------

/** Current indexing status of a repository. */
export type RepoStatus = "pending" | "indexing" | "ready" | "error";

/** A registered repository entry. */
export interface RepoRecord {
	/** UUID v4 identifier. */
	id: string;
	/** Human-readable name. */
	name: string;
	/** Absolute path to the repository root on disk. */
	path: string;
	/** Current indexing status. */
	status: RepoStatus;
	/** ISO 8601 creation timestamp. */
	created_at: string;
	/** ISO 8601 last-updated timestamp. */
	updated_at: string;
	/** ISO 8601 timestamp of the last successful index run. */
	last_indexed?: string;
	/** Human-readable error description when status === "Error". */
	error_message?: string;
}

/** Request body for POST /repositories */
export interface AddRepoRequest {
	/** Human-readable name for the repository. */
	name: string;
	/** Absolute or relative path to the repository root. */
	path: string;
}

/** Response body from GET /repositories */
export interface ListReposResponse {
	repositories: RepoRecord[];
	total: number;
}

/** Response body from GET /repositories/:id/status */
export interface RepoStatusResponse {
	id: string;
	status: RepoStatus;
	last_indexed?: string;
	error_message?: string;
}

// ---------------------------------------------------------------------------
// Embedding sample types
// ---------------------------------------------------------------------------

/** A single chunk represented as a projected 2D point. */
export interface EmbeddingSamplePoint {
	/** X coordinate in the projected space (approx −1 to 1). */
	x: number;
	/** Y coordinate in the projected space (approx −1 to 1). */
	y: number;
	/** Source file path. */
	file_path: string;
	/** Programming language, e.g. "rust". */
	language: string;
	/** Syntactic kind, e.g. "function". */
	chunk_type: string;
	/** Symbol name if present. */
	symbol_name?: string;
}

/** Response body from GET /repositories/:id/embeddings/sample */
export interface EmbeddingSampleResponse {
	/** Sampled points with 2D projection coordinates. */
	points: EmbeddingSamplePoint[];
	/** Total chunks in the repository (may be approximate). */
	total_chunks: number;
	/** Number of points actually returned. */
	sampled: number;
}
