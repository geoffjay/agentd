/**
 * Client for the Index service (default port 17012).
 *
 * Provides code search and repository management for the agentd-index service.
 */

import type { HealthResponse } from "@/types/common";
import type {
	AddRepoRequest,
	CodeAgenticSearchRequest,
	CodeAgenticSearchResponse,
	CodeSearchRequest,
	CodeSearchResponse,
	ListReposResponse,
	RepoRecord,
	RepoStatusResponse,
} from "@/types/codeindex";
import { ApiClient } from "./base";
import { serviceConfig } from "./config";

export class IndexClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	getHealth(): Promise<HealthResponse> {
		return this.get<HealthResponse>("/health");
	}

	// -------------------------------------------------------------------------
	// Search
	// -------------------------------------------------------------------------

	/** POST /search — vector/keyword/hybrid search over indexed code chunks. */
	search(request: CodeSearchRequest): Promise<CodeSearchResponse> {
		return this.post<CodeSearchResponse>("/search", request);
	}

	/** POST /search/agentic — grep-based fallback search over source files. */
	agenticSearch(
		request: CodeAgenticSearchRequest,
	): Promise<CodeAgenticSearchResponse> {
		return this.post<CodeAgenticSearchResponse>("/search/agentic", request);
	}

	// -------------------------------------------------------------------------
	// Repositories
	// -------------------------------------------------------------------------

	/** GET /repositories — list all registered repositories. */
	async listRepositories(): Promise<ListReposResponse> {
		const raw = await this.get<RepoRecord[] | ListReposResponse>("/repositories");
		// Backend returns array directly; normalise to ListReposResponse shape.
		if (Array.isArray(raw)) {
			return { repositories: raw, total: raw.length };
		}
		return raw as ListReposResponse;
	}

	/** POST /repositories — register a new repository. */
	addRepository(request: AddRepoRequest): Promise<RepoRecord> {
		return this.post<RepoRecord>("/repositories", request);
	}

	/** GET /repositories/:id — get a single repository by ID. */
	getRepository(id: string): Promise<RepoRecord> {
		return this.get<RepoRecord>(`/repositories/${id}`);
	}

	/** DELETE /repositories/:id — remove a repository. */
	deleteRepository(id: string): Promise<void> {
		return this.delete<void>(`/repositories/${id}`);
	}

	/** GET /repositories/:id/status — get repository indexing status. */
	getRepositoryStatus(id: string): Promise<RepoStatusResponse> {
		return this.get<RepoStatusResponse>(`/repositories/${id}/status`);
	}

	/** POST /repositories/:id/reindex — trigger re-indexing. */
	reindexRepository(id: string): Promise<RepoRecord> {
		return this.post<RepoRecord>(`/repositories/${id}/reindex`);
	}
}

/** Singleton client instance using the configured service URL. */
export const indexClient = new IndexClient({
	baseUrl: serviceConfig.indexServiceUrl,
});
