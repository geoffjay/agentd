/**
 * useApprovals -- unit tests.
 */

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const {
	mockListApprovals,
	mockListAgents,
	mockApproveRequest,
	mockDenyRequest,
} = vi.hoisted(() => ({
	mockListApprovals: vi.fn(),
	mockListAgents: vi.fn(),
	mockApproveRequest: vi.fn(),
	mockDenyRequest: vi.fn(),
}));

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		listApprovals: mockListApprovals,
		listAgents: mockListAgents,
		approveRequest: mockApproveRequest,
		denyRequest: mockDenyRequest,
	},
}));

import { useApprovals } from "@/hooks/useApprovals";

const APPROVALS = [
	{
		id: "ap-1",
		agent_id: "agent-1",
		tool_name: "Bash",
		tool_input: {},
		status: "pending",
		created_at: new Date().toISOString(),
	},
	{
		id: "ap-2",
		agent_id: "agent-2",
		tool_name: "Edit",
		tool_input: {},
		status: "pending",
		created_at: new Date().toISOString(),
	},
];

const AGENTS = [
	{ id: "agent-1", name: "builder" },
	{ id: "agent-2", name: "reviewer" },
];

describe("useApprovals", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockListApprovals.mockResolvedValue({ items: APPROVALS, total: 2 });
		mockListAgents.mockResolvedValue({ items: AGENTS, total: 2 });
		mockApproveRequest.mockResolvedValue({});
		mockDenyRequest.mockResolvedValue({});
	});

	it("fetches approvals and agents on mount", async () => {
		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.approvals).toHaveLength(2);
		expect(result.current.totalPendingCount).toBe(2);
		expect(result.current.agentMap.get("agent-1")?.name).toBe("builder");
	});

	it("sets error on fetch failure", async () => {
		mockListApprovals.mockRejectedValue(new Error("Server error"));

		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(result.current.error).toBe("Server error");
	});

	it("filters approvals by agentId", async () => {
		const { result } = renderHook(() =>
			useApprovals({ agentId: "agent-1", refreshInterval: 0 }),
		);

		await waitFor(() => expect(result.current.loading).toBe(false));
		expect(result.current.approvals).toHaveLength(1);
		expect(result.current.approvals[0].agent_id).toBe("agent-1");
		// totalPendingCount is still the full unfiltered count
		expect(result.current.totalPendingCount).toBe(2);
	});

	it("approve removes the approval from local state", async () => {
		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		await act(async () => {
			await result.current.approve("ap-1");
		});

		expect(mockApproveRequest).toHaveBeenCalledWith("ap-1");
		expect(result.current.approvals).toHaveLength(1);
		expect(result.current.approvals[0].id).toBe("ap-2");
	});

	it("deny removes the approval from local state", async () => {
		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		await act(async () => {
			await result.current.deny("ap-2");
		});

		expect(mockDenyRequest).toHaveBeenCalledWith("ap-2");
		expect(result.current.approvals).toHaveLength(1);
		expect(result.current.approvals[0].id).toBe("ap-1");
	});

	it("bulkApprove removes multiple approvals", async () => {
		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		await act(async () => {
			await result.current.bulkApprove(["ap-1", "ap-2"]);
		});

		expect(mockApproveRequest).toHaveBeenCalledTimes(2);
		expect(result.current.approvals).toHaveLength(0);
	});

	it("bulkDeny removes multiple approvals", async () => {
		const { result } = renderHook(() => useApprovals({ refreshInterval: 0 }));

		await waitFor(() => expect(result.current.loading).toBe(false));

		await act(async () => {
			await result.current.bulkDeny(["ap-1", "ap-2"]);
		});

		expect(mockDenyRequest).toHaveBeenCalledTimes(2);
		expect(result.current.approvals).toHaveLength(0);
	});
});
