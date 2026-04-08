/**
 * Tests for IndexClient (index service API client).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { IndexClient } from "@/services/codeindex";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeJsonResponse(status: number, body: unknown) {
	return new Response(JSON.stringify(body), {
		status,
		headers: new Headers({ "content-type": "application/json" }),
	});
}

function mockFetch(status: number, body: unknown) {
	vi.stubGlobal(
		"fetch",
		vi.fn().mockResolvedValue(makeJsonResponse(status, body)),
	);
}

function mockFetchEmpty(status: number) {
	vi.stubGlobal(
		"fetch",
		vi.fn().mockResolvedValue(
			new Response(null, {
				status,
				headers: new Headers({ "content-length": "0" }),
			}),
		),
	);
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const mockRepo = {
	id: "repo-1",
	name: "agentd",
	path: "/projects/agentd",
	status: "ready" as const,
	created_at: "2024-01-15T10:00:00Z",
	updated_at: "2024-01-15T10:00:00Z",
	last_indexed: "2024-01-15T11:00:00Z",
};

const mockResult = {
	id: "result-1",
	file_path: "src/main.rs",
	language: "rust",
	chunk_type: "function",
	symbol_name: "main",
	start_line: 1,
	end_line: 10,
	content: "fn main() {}",
	score: 0.9,
	repo_id: "repo-1",
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("IndexClient", () => {
	let client: IndexClient;

	beforeEach(() => {
		client = new IndexClient({
			baseUrl: "http://localhost:17012",
			maxRetries: 1,
		});
	});

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	describe("getHealth", () => {
		it("calls GET /health", async () => {
			mockFetch(200, { service: "index", version: "0.2.0", status: "ok" });
			const result = await client.getHealth();
			expect(result.service).toBe("index");

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/health");
		});
	});

	describe("search", () => {
		it("calls POST /search and returns results", async () => {
			mockFetch(200, { results: [mockResult], total: 1, query_time_ms: 42 });
			const result = await client.search({
				query: "main function",
				search_mode: "hybrid",
			});
			expect(result.results).toHaveLength(1);
			expect(result.total).toBe(1);
			expect(result.query_time_ms).toBe(42);

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/search");
			const init = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(init.method).toBe("POST");
			const body = JSON.parse(init.body as string);
			expect(body.query).toBe("main function");
			expect(body.search_mode).toBe("hybrid");
		});
	});

	describe("agenticSearch", () => {
		it("calls POST /search/agentic", async () => {
			mockFetch(200, { matches: [], total: 0, query_time_ms: 5 });
			const result = await client.agenticSearch({ query: "fn main" });
			expect(result.matches).toHaveLength(0);

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/search/agentic");
		});
	});

	describe("listRepositories", () => {
		it("normalises array response to ListReposResponse shape", async () => {
			mockFetch(200, [mockRepo]);
			const result = await client.listRepositories();
			expect(result.repositories).toHaveLength(1);
			expect(result.total).toBe(1);
		});

		it("passes through ListReposResponse shape unchanged", async () => {
			mockFetch(200, { repositories: [mockRepo], total: 1 });
			const result = await client.listRepositories();
			expect(result.repositories).toHaveLength(1);
			expect(result.total).toBe(1);
		});
	});

	describe("addRepository", () => {
		it("calls POST /repositories", async () => {
			mockFetch(201, mockRepo);
			const result = await client.addRepository({
				name: "agentd",
				path: "/projects/agentd",
			});
			expect(result.id).toBe("repo-1");

			const init = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(init.method).toBe("POST");
			const body = JSON.parse(init.body as string);
			expect(body.name).toBe("agentd");
		});
	});

	describe("getRepository", () => {
		it("calls GET /repositories/:id", async () => {
			mockFetch(200, mockRepo);
			const result = await client.getRepository("repo-1");
			expect(result.name).toBe("agentd");

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/repositories/repo-1");
		});
	});

	describe("deleteRepository", () => {
		it("calls DELETE /repositories/:id", async () => {
			mockFetchEmpty(204);
			await client.deleteRepository("repo-1");

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/repositories/repo-1");
			const init = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(init.method).toBe("DELETE");
		});
	});

	describe("getRepositoryStatus", () => {
		it("calls GET /repositories/:id/status", async () => {
			mockFetch(200, { id: "repo-1", status: "ready" });
			const result = await client.getRepositoryStatus("repo-1");
			expect(result.status).toBe("ready");

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/repositories/repo-1/status");
		});
	});

	describe("reindexRepository", () => {
		it("calls POST /repositories/:id/reindex", async () => {
			const indexing = { ...mockRepo, status: "indexing" as const };
			mockFetch(200, indexing);
			const result = await client.reindexRepository("repo-1");
			expect(result.status).toBe("indexing");

			const url = vi.mocked(fetch).mock.calls[0][0] as string;
			expect(url).toContain("/repositories/repo-1/reindex");
			const init = vi.mocked(fetch).mock.calls[0][1] as RequestInit;
			expect(init.method).toBe("POST");
		});
	});
});
