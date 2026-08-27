/**
 * Common shared types used across all agentd services.
 */

/** Standard paginated response wrapper */
export interface PaginatedResponse<T> {
	items: T[];
	total: number;
	limit: number;
	offset: number;
}

/** Standard health-check response */
export interface HealthResponse {
	service: string;
	version: string;
	status: string;
}

/** Execution backend type */
export type BackendType = "tmux" | "pty";

/** Information about the active execution backend and its capabilities */
export interface BackendInfo {
	backend_type: BackendType;
	version: string;
	/** Capability strings, e.g. "terminal", "interactive", "health-check" */
	capabilities: string[];
}

/** Typed API error thrown by all client methods */
export class ApiError extends Error {
	readonly status: number;
	readonly body: unknown;

	constructor(status: number, message: string, body?: unknown) {
		super(message);
		this.name = "ApiError";
		this.status = status;
		this.body = body;
	}
}

/** Pagination query parameters accepted by list endpoints */
export interface PaginationParams {
	limit?: number;
	offset?: number;
}

/** Generic status-filter params (extends pagination) */
export interface StatusFilterParams extends PaginationParams {
	status?: string;
}
