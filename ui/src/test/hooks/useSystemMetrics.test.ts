/**
 * useSystemMetrics -- MSW integration tests.
 *
 * Note: the ApiClient retries on 5xx errors, so the service-down tests use
 * 4xx responses to avoid slow retries (matching serviceHealth.test.tsx).
 */

import { renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { describe, expect, it } from "vitest";
import { useSystemMetrics } from "@/hooks/useSystemMetrics";
import { makeMonitorAlert, makeSystemStatus } from "@/test/mocks/factories";
import { server } from "@/test/mocks/server";

const BASE = "http://localhost:17003";

describe("useSystemMetrics (MSW integration)", () => {
	it("loads history and status on the happy path", async () => {
		const { result } = renderHook(() => useSystemMetrics());

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.available).toBe(true);
		expect(result.current.error).toBeUndefined();
		expect(result.current.history).toHaveLength(12);
		expect(result.current.latest).not.toBeNull();
		expect(result.current.latest?.cpu.usage_percent).toBeCloseTo(42.5);
		expect(result.current.latest?.memory.usage_percent).toBeCloseTo(50.0);
		expect(result.current.status).toBe("healthy");
		expect(result.current.alerts).toHaveLength(0);
	});

	it("surfaces threshold alerts from the status endpoint", async () => {
		server.use(
			http.get(`${BASE}/status`, () =>
				HttpResponse.json(
					makeSystemStatus({
						status: "degraded",
						alerts: [makeMonitorAlert()],
					}),
				),
			),
		);

		const { result } = renderHook(() => useSystemMetrics());
		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.status).toBe("degraded");
		expect(result.current.alerts).toHaveLength(1);
		expect(result.current.alerts[0].metric).toBe("cpu");
	});

	it("marks the service unavailable when all endpoints fail", async () => {
		server.use(
			http.get(`${BASE}/history`, () =>
				HttpResponse.json({ error: "Not found" }, { status: 404 }),
			),
			http.get(`${BASE}/status`, () =>
				HttpResponse.json({ error: "Not found" }, { status: 404 }),
			),
		);

		const { result } = renderHook(() => useSystemMetrics());
		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.available).toBe(false);
		expect(result.current.error).toBe("Monitor service unavailable");
		expect(result.current.history).toHaveLength(0);
		expect(result.current.latest).toBeNull();
	});

	it("still returns history when only the status endpoint fails", async () => {
		server.use(
			http.get(`${BASE}/status`, () =>
				HttpResponse.json({ error: "Not found" }, { status: 404 }),
			),
		);

		const { result } = renderHook(() => useSystemMetrics());
		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.available).toBe(true);
		expect(result.current.history).toHaveLength(12);
		// Falls back to the newest history entry for `latest`
		expect(result.current.latest).not.toBeNull();
		expect(result.current.status).toBeNull();
	});
});
