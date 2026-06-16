/**
 * TypeScript types for the Knowledge service.
 * Mirrors the Rust types in crates/knowledge/src/types.rs.
 */

// ---------------------------------------------------------------------------
// Document model
// ---------------------------------------------------------------------------

/** A single document record stored in the knowledge service. */
export interface Document {
	/** Unique identifier (UUID). */
	id: string;
	/** Project this document belongs to (UUID from orchestrator). */
	project_id: string;
	/** Optional organization scope (null until tenant scoping lands). */
	organization_id: string | null;
	/** Relative filesystem path, e.g. "docs/api.md". Must end in .md. */
	rel_path: string;
	/** Human-readable title (defaults to filename stem). */
	title: string;
	/** Size of the markdown file on disk in bytes. */
	size_bytes: number;
	/** RFC 3339 creation timestamp. */
	created_at: string;
	/** RFC 3339 last-updated timestamp. */
	updated_at: string;
}

/** Document metadata plus the raw markdown body. */
export interface DocumentContent {
	document: Document;
	content: string;
}

// ---------------------------------------------------------------------------
// Tree model
// ---------------------------------------------------------------------------

/** A node in the virtual folder/file tree for a project. */
export type TreeNode =
	| {
			type: "folder";
			name: string;
			path: string;
			children: TreeNode[];
	  }
	| {
			type: "file";
			name: string;
			path: string;
			doc_id: string;
	  };

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/** Request body for `POST /projects/:id/documents`. */
export interface CreateDocumentRequest {
	/** Relative path (must end in .md, depth <= 8). */
	rel_path: string;
	/** Optional title override (defaults to filename stem). */
	title?: string;
	/** Markdown body. */
	content: string;
}

/** Request body for `PUT /projects/:id/documents/:doc_id`. */
export interface UpdateDocumentRequest {
	/** New markdown body (omit to leave unchanged). */
	content?: string;
	/** New title (omit to leave unchanged). */
	title?: string;
	/**
	 * Optimistic concurrency token — the `updated_at` value returned by the
	 * last GET. The server rejects the update if the document has been modified
	 * since this timestamp.
	 */
	expected_updated_at?: string;
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

/** Query parameters accepted by `GET /projects/:id/documents`. */
export interface DocumentListParams {
	/** Filter to documents whose rel_path starts with this prefix. */
	prefix?: string;
	/** Maximum items to return (default 50, max 200). */
	limit?: number;
	/** Pagination offset (default 0). */
	offset?: number;
}
