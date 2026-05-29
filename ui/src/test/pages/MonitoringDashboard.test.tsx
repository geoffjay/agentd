/**
 * MonitoringDashboard — smoke tests for the monitoring page.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/hooks/useMetrics", () => ({
	useMetrics: () => ({
		agentCounts: { running: 3, pending: 1, stopped: 0, failed: 0 },
		notifCounts: { pending: 2, actionable: 1, total: 10 },
		serviceMetrics: [
			{
				key: "orchestrator",
				name: "Orchestrator",
				port: 17006,
				status: "healthy",
				http: { requestRate: 10, errorRate: 0, latencyP50: 12 },
			},
			{
				key: "notify",
				name: "Notify",
				port: 17004,
				status: "healthy",
				http: { requestRate: 5, errorRate: 0, latencyP50: 8 },
			},
			{
				key: "ask",
				name: "Ask",
				port: 17001,
				status: "healthy",
				http: { requestRate: 2, errorRate: 0, latencyP50: 15 },
			},
		],
		agentTimeSeries: [],
		loading: false,
		lastRefresh: new Date(),
		error: undefined,
		refetch: vi.fn(),
		refreshInterval: 30_000,
		setRefreshInterval: vi.fn(),
	}),
}));

vi.mock("@/hooks/useUsageMetrics", () => ({
	useUsageMetrics: () => ({
		entries: [],
		aggregate: {
			totalInputTokens: 1000,
			totalOutputTokens: 500,
			totalCacheReadTokens: 200,
			totalCacheCreationTokens: 100,
			totalCost: 0.05,
			cacheHitRatio: 0.2,
		},
		loading: false,
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

// Mock theme hooks to avoid ThemeProvider dependency in unit tests
vi.mock("@/hooks/useTheme", () => ({
	useTheme: () => ({
		themeId: "agentd-dark",
		resolvedThemeId: "agentd-dark",
		theme: {},
		setTheme: vi.fn(),
	}),
	ThemeProvider: ({ children }: { children: React.ReactNode }) => (
		<>{children}</>
	),
}));

vi.mock("@/hooks/useNivoTheme", () => ({ useNivoTheme: () => ({}) }));

// Mock nivo charts to avoid ResizeObserver issues in jsdom
vi.mock("@nivo/bar", () => ({
	ResponsiveBar: () => <div data-testid="mock-bar" />,
}));
vi.mock("@nivo/pie", () => ({
	ResponsivePie: () => <div data-testid="mock-pie" />,
}));
vi.mock("@nivo/line", () => ({
	ResponsiveLine: () => <div data-testid="mock-line" />,
}));

import { MonitoringDashboard } from "@/pages/monitoring/MonitoringDashboard";

function renderPage() {
	return render(
		<MemoryRouter>
			<MonitoringDashboard />
		</MemoryRouter>,
	);
}

describe("MonitoringDashboard", () => {
	it("renders the page heading", () => {
		renderPage();
		expect(screen.getByText("Monitoring")).toBeInTheDocument();
	});

	it("renders refresh interval options", () => {
		renderPage();
		expect(screen.getByText("10s")).toBeInTheDocument();
		expect(screen.getByText("30s")).toBeInTheDocument();
		expect(screen.getByText("1m")).toBeInTheDocument();
		expect(screen.getByText("5m")).toBeInTheDocument();
	});

	it("renders a manual refresh button", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /refresh metrics/i }),
		).toBeInTheDocument();
	});
});
