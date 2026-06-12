/**
 * usePlatformMetrics -- MSW integration tests.
 *
 * Note: the ApiClient retries 5xx responses with backoff, so the 502 test
 * pays ~600ms of retries per cycle — acceptable, but don't add more 502
 * cases than needed.
 */

import { renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { describe, expect, it } from "vitest";
import {
	PLATFORM_QUERIES,
	usePlatformMetrics,
	vectorEntries,
	vectorValue,
} from "@/hooks/usePlatformMetrics";
import {
	makeVectorQueryResult,
	makeVectorSample,
} from "@/test/mocks/factories";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17003";

describe("usePlatformMetrics (MSW integration)", () => {
	it("loads every platform query on the happy path", async () => {
		const { result } = renderHook(() => usePlatformMetrics());

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.monitorDown).toBe(false);
		expect(result.current.prometheusDown).toBe(false);
		for (const name of PLATFORM_QUERIES) {
			expect(result.current.results[name]).toBeDefined();
		}
	});

	it("flags prometheusDown on 502 responses", async () => {
		server.use(
			http.get(`${BASE}/queries/:name`, () =>
				HttpResponse.json(
					{ error: "Prometheus unavailable: connection refused" },
					{ status: 502 },
				),
			),
		);

		const { result } = renderHook(() => usePlatformMetrics());
		await waitFor(() => expect(result.current.loading).toBe(false), {
			timeout: 10_000,
		});

		expect(result.current.prometheusDown).toBe(true);
		expect(result.current.monitorDown).toBe(false);
		expect(Object.keys(result.current.results)).toHaveLength(0);
	}, 15_000);

	it("flags monitorDown when nothing answers", async () => {
		// 404s avoid the ApiClient's 5xx retry backoff (see header note).
		server.use(
			http.get(`${BASE}/queries/:name`, () =>
				HttpResponse.json({ error: "Not found" }, { status: 404 }),
			),
		);

		const { result } = renderHook(() => usePlatformMetrics());
		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.monitorDown).toBe(true);
		expect(result.current.prometheusDown).toBe(false);
	});
});

describe("vector helpers", () => {
	it("vectorEntries extracts labels and numeric values", () => {
		const result = makeVectorQueryResult("http-p95-latency", [
			makeVectorSample({
				metric: { service: "orchestrator" },
				value: [1, "0.0125"],
			}),
			makeVectorSample({ metric: { service: "notify" }, value: [1, "0.008"] }),
		]);

		const entries = vectorEntries(result);
		expect(entries).toHaveLength(2);
		expect(entries[0].labels.service).toBe("orchestrator");
		expect(entries[0].value).toBeCloseTo(0.0125);
	});

	it("vectorValue sums samples and returns null for empty/missing data", () => {
		const result = makeVectorQueryResult("dispatch-throughput", [
			makeVectorSample({ metric: { status: "completed" }, value: [1, "12"] }),
			makeVectorSample({ metric: { status: "failed" }, value: [1, "2"] }),
		]);

		expect(vectorValue(result)).toBe(14);
		expect(vectorValue(makeVectorQueryResult("x", []))).toBeNull();
		expect(vectorValue(undefined)).toBeNull();
	});
});
