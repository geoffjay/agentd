/**
 * TypeScript types for the Memory service.
 * Mirrors the Rust types in crates/memory/src/types/.
 */

// ---------------------------------------------------------------------------
// Enums / union types
// ---------------------------------------------------------------------------

/** Semantic category of a memory record.
 *
 * Mirrors the Rust `MemoryType` enum serialized as lowercase strings.
 */
export type MemoryType = 'information' | 'question' | 'request'

/** Access-control tier for a memory record.
 *
 * Mirrors the Rust `VisibilityLevel` enum:
 * - `public`  — visible to everyone (including anonymous)
 * - `shared`  — visible to creator, owner, and actors in `shared_with`
 * - `private` — visible to creator and owner only
 */
export type VisibilityLevel = 'public' | 'shared' | 'private'

// ---------------------------------------------------------------------------
// Memory model
// ---------------------------------------------------------------------------

/** A single memory record stored in the system.
 *
 * Mirrors the Rust `Memory` struct. The `type` field is the JSON-serialized
 * form of the Rust `memory_type` field (renamed via `#[serde(rename = "type")]`).
 */
export interface Memory {
  /** Unique identifier in the format `mem_<unix_ms>_<8-char-uuid-prefix>`. */
  id: string
  /** The natural-language content of the memory. */
  content: string
  /** Semantic category of this memory record. */
  type: MemoryType
  /** Free-form tags for filtering and organisation. */
  tags: string[]
  /** The actor (agent or user identifier) that created this memory. */
  created_by: string
  /** Optional owner; may differ from `created_by`. */
  owner?: string
  /** Timestamp when this memory was first stored (RFC 3339). */
  created_at: string
  /** Timestamp of the most recent mutation (RFC 3339). */
  updated_at: string
  /** Access-control tier for this memory. */
  visibility: VisibilityLevel
  /** Actors allowed to read this memory when visibility is `shared`. */
  shared_with: string[]
  /** IDs of other memories that this record references or relates to. */
  references: string[]
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

/** Request body for `POST /memories` — create a new memory record.
 *
 * Only `content` and `created_by` are required; all other fields have
 * sensible defaults on the server side.
 */
export interface CreateMemoryRequest {
  /** The natural-language content to store. */
  content: string
  /** Identity of the actor creating this memory. */
  created_by: string
  /** Semantic category (defaults to `information`). */
  type?: MemoryType
  /** Free-form tags for filtering. */
  tags?: string[]
  /** Access control tier (defaults to `public`). */
  visibility?: VisibilityLevel
  /** Actors to share with when `visibility` is `shared`. */
  shared_with?: string[]
  /** IDs of related memories this record references. */
  references?: string[]
}

/** Request body for `POST /memories/search` — semantic similarity search.
 *
 * All filters are optional and are ANDed together.
 */
export interface SearchRequest {
  /** Natural-language query string used for vector similarity search. */
  query: string
  /** Actor performing the search; used to filter by visibility. */
  as_actor?: string
  /** Restrict results to a specific memory type. */
  type?: MemoryType
  /** Restrict results to memories that have at least one of these tags. */
  tags?: string[]
  /** Return only memories created on or after this timestamp (RFC 3339). */
  from?: string
  /** Return only memories created on or before this timestamp (RFC 3339). */
  to?: string
  /** Maximum number of results to return (defaults to 10). */
  limit?: number
}

/** Request body for `PUT /memories/:id/visibility`. */
export interface UpdateVisibilityRequest {
  /** New visibility level to apply. */
  visibility: VisibilityLevel
  /** Updated share list. Required when `visibility` is `shared`. */
  shared_with?: string[]
  /** Optional actor identity for ownership verification. */
  as_actor?: string
}

// ---------------------------------------------------------------------------
// Response bodies
// ---------------------------------------------------------------------------

/** Response envelope for search results from `POST /memories/search`. */
export interface SearchResponse {
  /** Matching memory records, ordered by similarity score. */
  memories: Memory[]
  /** Total number of matches. */
  total: number
}

/** Response body for `DELETE /memories/:id`. */
export interface DeleteResponse {
  /** `true` if a record was actually removed; `false` if the ID was not found. */
  deleted: boolean
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

/** Query parameters for `GET /memories` list endpoint. */
export interface MemoryListParams {
  /** Filter by memory type (`information`, `question`, `request`). */
  type?: MemoryType
  /** Filter by tag. */
  tag?: string
  /** Filter by creator identity. */
  created_by?: string
  /** Filter by visibility level. */
  visibility?: VisibilityLevel
  /** Maximum number of items to return (default: 50, max: 200). */
  limit?: number
  /** Number of items to skip (default: 0). */
  offset?: number
}
