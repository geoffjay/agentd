/**
 * DashboardPage — Create Agent button wiring tests.
 *
 * The dashboard's "Create new agent" button navigates to the dedicated
 * /agents/new page (the old slide-over dialog was removed).
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

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
		refetch: vi.fn(),
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

vi.mock("@/hooks/useSystemMetrics", () => ({
	useSystemMetrics: () => ({
		history: [],
		latest: null,
		alerts: [],
		status: null,
		available: false,
		loading: false,
		error: "Monitor service unavailable",
		refetch: vi.fn(),
	}),
}));

vi.mock("@/hooks/useDashboardStats", () => ({
	useDashboardStats: () => ({
		pendingApprovals: 0,
		workflows: 0,
		pendingQuestions: 0,
		loading: false,
		refetch: vi.fn(),
	}),
}));

vi.mock("@/hooks/useActivityFeed", () => ({
	useActivityFeed: () => ({
		events: [],
		buckets: [],
		loading: false,
		error: undefined,
		refetch: vi.fn(),
	}),
}));

// Mock nivo so it doesn't complain about missing ResizeObserver in jsdom
vi.mock("@nivo/pie", () => ({
	ResponsivePie: () => <div data-testid="mock-pie" />,
}));
vi.mock("@nivo/bar", () => ({
	ResponsiveBar: () => <div data-testid="mock-bar" />,
}));
vi.mock("@nivo/line", () => ({
	ResponsiveLine: () => <div data-testid="mock-line" />,
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
		<MemoryRouter initialEntries={["/"]}>
			<Routes>
				<Route path="/" element={<DashboardPage />} />
				<Route
					path="/agents/new"
					element={<div data-testid="agent-form-page" />}
				/>
			</Routes>
		</MemoryRouter>,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DashboardPage — Create Agent button", () => {
	it("renders the Create Agent button inside AgentSummary", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /create new agent/i }),
		).toBeInTheDocument();
	});

	it("navigates to /agents/new when the button is clicked", () => {
		renderPage();
		expect(screen.queryByTestId("agent-form-page")).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: /create new agent/i }));

		expect(screen.getByTestId("agent-form-page")).toBeInTheDocument();
	});
});
