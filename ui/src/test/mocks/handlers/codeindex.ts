/**
 * MSW request handlers for the Index service (port 17012).
 *
 * Provides default responses for health, repositories, and search endpoints.
 * Override per test with server.use().
 */

import { HttpResponse, http } from "msw";
import {
	makeCodeSearchResponse,
	makeRepoRecord,
	makeRepoList,
	makeRepoStatusResponse,
	makeAgenticSearchResponse,
} from "../factories/codeindex";

const BASE = "http://localhost:17012";

const DEFAULT_REPOS = makeRepoList(2);

export const codeIndexHandlers = [
	// Health
	http.get(`${BASE}/health`, () =>
		HttpResponse.json({ status: "ok", service: "index", version: "0.2.0" }),
	),

	// List repositories
	http.get(`${BASE}/repositories`, () =>
		HttpResponse.json(DEFAULT_REPOS),
	),

	// Add repository
	http.post(`${BASE}/repositories`, async ({ request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		const repo = makeRepoRecord({
			name: String(body.name ?? "new-repo"),
			path: String(body.path ?? "/projects/new-repo"),
			status: "pending",
		});
		return HttpResponse.json(repo, { status: 201 });
	}),

	// Get repository by id
	http.get(`${BASE}/repositories/:id`, ({ params }) => {
		const repo =
			DEFAULT_REPOS.find((r) => r.id === params.id) ??
			makeRepoRecord({ id: String(params.id) });
		return HttpResponse.json(repo);
	}),

	// Delete repository
	http.delete(`${BASE}/repositories/:id`, () =>
		new HttpResponse(null, { status: 204 }),
	),

	// Repository status
	http.get(`${BASE}/repositories/:id/status`, ({ params }) =>
		HttpResponse.json(makeRepoStatusResponse({ id: String(params.id) })),
	),

	// Reindex repository
	http.post(`${BASE}/repositories/:id/reindex`, ({ params }) => {
		const repo =
			DEFAULT_REPOS.find((r) => r.id === params.id) ??
			makeRepoRecord({ id: String(params.id) });
		return HttpResponse.json({ ...repo, status: "indexing" });
	}),

	// Vector / keyword / hybrid search
	http.post(`${BASE}/search`, () =>
		HttpResponse.json(makeCodeSearchResponse()),
	),

	// Agentic search
	http.post(`${BASE}/search/agentic`, () =>
		HttpResponse.json(makeAgenticSearchResponse()),
	),
];
