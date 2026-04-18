/**
 * useSystemAgents -- unit tests.
 */

import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockListSystemAgents = vi.hoisted(() => vi.fn());

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		listSystemAgents: mockListSystemAgents,
	},
}));

import { useSystemAgents } from "@/hooks/useSystemAgents";

const SYSTEM_AGENTS = [
	{ id: "sys-scheduler", name: "scheduler", status: "running" },
	{ id: "sys-monitor", name: "monitor", status: "running" },
];

describe("useSystemAgents", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		vi.useFakeTimers();
		mockListSystemAgents.mockResolvedValue(SYSTEM_AGENTS);
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("fetches system agents on mount", async () => {
		vi.useRealTimers();
		const { result } = renderHook(() => useSystemAgents({ paused: true }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.agents).toEqual(SYSTEM_AGENTS);
		expect(mockListSystemAgents).toHaveBeenCalledTimes(1);
	});

	it("sets error on fetch failure", async () => {
		vi.useRealTimers();
		mockListSystemAgents.mockRejectedValue(new Error("Network error"));

		const { result } = renderHook(() => useSystemAgents({ paused: true }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.error).toContain("Network error");
		expect(result.current.agents).toEqual([]);
	});

	it("starts in loading state", () => {
		const { result } = renderHook(() => useSystemAgents({ paused: true }));
		expect(result.current.loading).toBe(true);
	});
});
