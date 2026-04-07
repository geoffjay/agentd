/**
 * Tests for useIndexService hook.
 *
 * Uses MSW to intercept API calls so the full client path is exercised.
 */

import { act, renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { beforeEach, describe, expect, it } from "vitest";
import { useIndexService } from "@/hooks/useIndexService";
import {
	makeCodeSearchResponse,
	makeRepoList,
	makeRepoRecord,
	resetCodeIndexSeq,
} from "@/test/mocks/factories/codeindex";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17012";

beforeEach(() => {
	resetCodeIndexSeq();
});

describe("useIndexService", () => {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	describe("health check", () => {
		it("sets reachable true on success", async () => {
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", version: "0.2.0", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.health.checking).toBe(false));

			expect(result.current.health.reachable).toBe(true);
			expect(result.current.health.version).toBe("0.2.0");
		});

		it("sets reachable false when health endpoint errors", async () => {
			server.use(
				http.get(`${BASE}/health`, () => HttpResponse.error()),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.health.checking).toBe(false));

			expect(result.current.health.reachable).toBe(false);
		});

		it("recheckHealth re-runs the health check", async () => {
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", version: "0.2.0", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.health.checking).toBe(false));
			expect(result.current.health.reachable).toBe(true);

			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", version: "0.3.0", status: "ok" }),
				),
			);

			await act(async () => { result.current.recheckHealth(); });
			await waitFor(() => expect(result.current.health.version).toBe("0.3.0"));
		});
	});

	// -------------------------------------------------------------------------
	// Repositories
	// -------------------------------------------------------------------------

	describe("repositories", () => {
		it("fetches and returns repositories on mount", async () => {
			const repos = makeRepoList(2);
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json(repos)),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			expect(result.current.repositories).toHaveLength(2);
		});

		it("sets reposError on fetch failure", async () => {
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.error()),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			expect(result.current.reposError).toBeDefined();
			expect(result.current.repositories).toHaveLength(0);
		});

		it("addRepository appends to the list", async () => {
			const existing = makeRepoRecord();
			const newRepo = makeRepoRecord({ name: "new-repo" });
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([existing])),
				http.post(`${BASE}/repositories`, () =>
					HttpResponse.json(newRepo, { status: 201 }),
				),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));
			expect(result.current.repositories).toHaveLength(1);

			let ok = false;
			await act(async () => {
				ok = await result.current.addRepository({
					name: "new-repo",
					path: "/projects/new-repo",
				});
			});

			expect(ok).toBe(true);
			expect(result.current.repositories).toHaveLength(2);
		});

		it("addRepository returns false and sets error on API failure", async () => {
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
				http.post(`${BASE}/repositories`, () => HttpResponse.error()),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			let ok = true;
			await act(async () => {
				ok = await result.current.addRepository({ name: "bad", path: "/bad" });
			});

			expect(ok).toBe(false);
			expect(result.current.reposError).toBeDefined();
		});

		it("deleteRepository removes the repo from the list", async () => {
			const repo = makeRepoRecord({ id: "repo-del" });
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([repo])),
				http.delete(`${BASE}/repositories/repo-del`, () =>
					new HttpResponse(null, { status: 204 }),
				),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));
			expect(result.current.repositories).toHaveLength(1);

			await act(async () => {
				await result.current.deleteRepository("repo-del");
			});

			expect(result.current.repositories).toHaveLength(0);
		});

		it("reindexRepository updates the repo status in the list", async () => {
			const repo = makeRepoRecord({ id: "repo-ri", status: "Ready" });
			const updated = { ...repo, status: "Indexing" as const };
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([repo])),
				http.post(`${BASE}/repositories/repo-ri/reindex`, () =>
					HttpResponse.json(updated),
				),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			await act(async () => {
				await result.current.reindexRepository("repo-ri");
			});

			expect(result.current.repositories[0].status).toBe("Indexing");
		});
	});

	// -------------------------------------------------------------------------
	// Search
	// -------------------------------------------------------------------------

	describe("search", () => {
		it("runSearch populates searchResults", async () => {
			const resp = makeCodeSearchResponse(undefined, { query_time_ms: 77 });
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
				http.post(`${BASE}/search`, () => HttpResponse.json(resp)),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			await act(async () => {
				await result.current.runSearch({ query: "fn main", search_mode: "Hybrid" });
			});

			expect(result.current.searchResults).toHaveLength(resp.results.length);
			expect(result.current.searchTotal).toBe(resp.total);
			expect(result.current.searchQueryMs).toBe(77);
			expect(result.current.searchError).toBeUndefined();
		});

		it("runSearch sets searchError on failure", async () => {
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
				http.post(`${BASE}/search`, () => HttpResponse.error()),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			await act(async () => {
				await result.current.runSearch({ query: "fn main", search_mode: "Vector" });
			});

			expect(result.current.searchError).toBeDefined();
			expect(result.current.searchResults).toHaveLength(0);
		});

		it("clearSearch resets all search state", async () => {
			const resp = makeCodeSearchResponse();
			server.use(
				http.get(`${BASE}/health`, () =>
					HttpResponse.json({ service: "index", status: "ok" }),
				),
				http.get(`${BASE}/repositories`, () => HttpResponse.json([])),
				http.post(`${BASE}/search`, () => HttpResponse.json(resp)),
			);

			const { result } = renderHook(() => useIndexService());
			await waitFor(() => expect(result.current.reposLoading).toBe(false));

			await act(async () => {
				await result.current.runSearch({ query: "test", search_mode: "Keyword" });
			});
			expect(result.current.searchResults.length).toBeGreaterThan(0);

			act(() => result.current.clearSearch());

			expect(result.current.searchResults).toHaveLength(0);
			expect(result.current.searchTotal).toBe(0);
			expect(result.current.searchQueryMs).toBeUndefined();
		});
	});
});
