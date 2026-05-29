/**
 * DashboardPage — Create Agent button wiring tests.
 *
 * Verifies that clicking the "Create Agent" button opens the dialog,
 * that submitting calls orchestratorClient.createAgent, and that the
 * dialog can be closed.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Mock all hooks the page depends on so the test is fast and deterministic
// ---------------------------------------------------------------------------

const mockRefetch = vi.fn();

vi.mock("@/hooks/useAgentSummary", () => ({
	useAgentSummary: () => ({
		counts: { running: 0, pending: 0, stopped: 0, failed: 0 },
		recentAgents: [],
		total: 0,
		aggregateUsage: null,
		loading: false,
		error: undefined,
		refetch: mockRefetch,
	}),
}));

vi.mock("@/hooks/useNotificationSummary", () => ({
	useNotificationSummary: () => ({
		pending: 0,
		unread: 0,
		total: 0,
		priorityCounts: { low: 0, normal: 0, high: 0, urgent: 0 },
		loading: false,
		error: undefined,
	}),
}));

vi.mock("@/hooks/useServiceHealth", () => ({
	useServiceHealth: () => ({
		services: [],
		loading: false,
		initializing: false,
		refresh: vi.fn(),
	}),
}));

// Mock nivo so it doesn't complain about missing ResizeObserver in jsdom
vi.mock("@nivo/pie", () => ({
	ResponsivePie: () => <div data-testid="mock-pie" />,
}));

// ---------------------------------------------------------------------------
// Mock orchestratorClient.createAgent
// ---------------------------------------------------------------------------

const mockCreateAgent = vi.fn();

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		createAgent: (...args: unknown[]) => mockCreateAgent(...args),
	},
}));

// ---------------------------------------------------------------------------
// Import the page AFTER all mocks are registered
// ---------------------------------------------------------------------------

import { DashboardPage } from "@/pages/DashboardPage";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderPage() {
	return render(
		<MemoryRouter>
			<DashboardPage />
		</MemoryRouter>,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DashboardPage — Create Agent button", () => {
	beforeEach(() => {
		mockCreateAgent.mockResolvedValue({ id: "agent-1", name: "test" });
		mockRefetch.mockClear();
	});

	it("renders the Create Agent button inside AgentSummary", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /create new agent/i }),
		).toBeInTheDocument();
	});

	it("opens the CreateAgentDialog when the button is clicked", () => {
		renderPage();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /create new agent/i }));

		expect(screen.getByRole("dialog")).toBeInTheDocument();
		expect(screen.getByLabelText("Create Agent")).toBeInTheDocument();
	});

	it("closes the dialog when Cancel is clicked", () => {
		renderPage();
		fireEvent.click(screen.getByRole("button", { name: /create new agent/i }));

		expect(screen.getByRole("dialog")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /cancel/i }));

		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("calls orchestratorClient.createAgent and refetch on submit", async () => {
		renderPage();
		fireEvent.click(screen.getByRole("button", { name: /create new agent/i }));

		// Fill in required fields
		fireEvent.change(screen.getByLabelText(/^name$/i), {
			target: { value: "my-agent" },
		});
		fireEvent.change(screen.getByLabelText(/working directory/i), {
			target: { value: "/home/user/project" },
		});

		fireEvent.click(screen.getByRole("button", { name: /^create agent$/i }));

		await waitFor(() => {
			expect(mockCreateAgent).toHaveBeenCalledWith(
				expect.objectContaining({ name: "my-agent" }),
			);
		});

		expect(mockRefetch).toHaveBeenCalled();

		// Dialog should close after successful submit
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
});
