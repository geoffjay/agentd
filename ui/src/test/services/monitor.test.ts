/**
 * Tests for MonitorClient — focused on the named Prometheus query API
 * (GET /queries, GET /queries/{name}).
 */

import { afterEach, describe, expect, it, vi } from "vitest";
import { MonitorClient } from "@/services/monitor";
import { ApiError } from "@/types/common";
import {
	makeQueryCatalog,
	makeVectorQueryResult,
} from "../mocks/factories/monitor";

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
	const fetchMock = vi.fn().mockResolvedValue(makeJsonResponse(status, body));
	vi.stubGlobal("fetch", fetchMock);
	return fetchMock;
}

function makeClient() {
	// maxRetries: 1 — error-path tests must not sit through retry backoff.
	return new MonitorClient({
		baseUrl: "http://localhost:17003",
		maxRetries: 1,
	});
}

afterEach(() => {
	vi.unstubAllGlobals();
});

// ---------------------------------------------------------------------------
// listQueries
// ---------------------------------------------------------------------------

describe("MonitorClient.listQueries", () => {
	it("fetches the catalog from GET /queries", async () => {
		const catalog = makeQueryCatalog();
		const fetchMock = mockFetch(200, catalog);

		const result = await makeClient().listQueries();

		expect(fetchMock).toHaveBeenCalledTimes(1);
		const url = String(fetchMock.mock.calls[0][0]);
		expect(url).toBe("http://localhost:17003/queries");
		expect(result).toHaveLength(catalog.length);
		expect(result[0].name).toBe("dispatch-success-rate");
	});
});

// ---------------------------------------------------------------------------
// runQuery
// ---------------------------------------------------------------------------

describe("MonitorClient.runQuery", () => {
	it("fetches GET /queries/{name} with no params by default", async () => {
		const fetchMock = mockFetch(200, makeVectorQueryResult("agents-active"));

		const result = await makeClient().runQuery("agents-active");

		const url = new URL(String(fetchMock.mock.calls[0][0]));
		expect(url.pathname).toBe("/queries/agents-active");
		expect([...url.searchParams.keys()]).toHaveLength(0);
		expect(result.data.resultType).toBe("vector");
	});

	it("encodes window/mode/range params", async () => {
		const fetchMock = mockFetch(
			200,
			makeVectorQueryResult("dispatch-success-rate"),
		);

		await makeClient().runQuery("dispatch-success-rate", {
			window: "15m",
			mode: "range",
			rangeMinutes: 120,
			stepSecs: 30,
		});

		const url = new URL(String(fetchMock.mock.calls[0][0]));
		expect(url.searchParams.get("window")).toBe("15m");
		expect(url.searchParams.get("mode")).toBe("range");
		expect(url.searchParams.get("range_minutes")).toBe("120");
		expect(url.searchParams.get("step_secs")).toBe("30");
	});

	it("URL-encodes the query name", async () => {
		const fetchMock = mockFetch(200, makeVectorQueryResult("x"));

		await makeClient().runQuery("weird/name");

		const url = String(fetchMock.mock.calls[0][0]);
		expect(url).toContain("/queries/weird%2Fname");
	});

	it("propagates 502 (Prometheus down) as ApiError with status 502", async () => {
		mockFetch(502, { error: "Prometheus unavailable: connection refused" });

		const err = await makeClient()
			.runQuery("agents-active")
			.catch((e: unknown) => e);

		expect(err).toBeInstanceOf(ApiError);
		expect((err as ApiError).status).toBe(502);
		expect((err as ApiError).message).toContain("Prometheus unavailable");
	});

	it("propagates 404 (unknown query) with the server's catalog hint", async () => {
		mockFetch(404, {
			error: "Unknown query `bogus`. Known queries: dispatch-success-rate",
		});

		const err = await makeClient()
			.runQuery("bogus")
			.catch((e: unknown) => e);

		expect(err).toBeInstanceOf(ApiError);
		expect((err as ApiError).status).toBe(404);
		expect((err as ApiError).message).toContain("dispatch-success-rate");
	});
});
