/**
 * MonitoringDashboard — smoke tests for the monitoring page.
 */

import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

const mockUseSystemMetrics = vi.fn();
vi.mock("@/hooks/useSystemMetrics", () => ({
	useSystemMetrics: () => mockUseSystemMetrics(),
}));

vi.mock("@/hooks/usePlatformMetrics", async (importOriginal) => {
	// Keep the real vector helpers — only the hook itself is stubbed.
	const original =
		await importOriginal<typeof import("@/hooks/usePlatformMetrics")>();
	return {
		...original,
		usePlatformMetrics: () => ({
			results: {},
			monitorDown: false,
			prometheusDown: false,
			loading: false,
			refetch: vi.fn(),
		}),
	};
});

function systemMetricsResult(overrides?: Record<string, unknown>) {
	const snapshot = {
		collected_at: "2024-01-01T00:00:00Z",
		cpu: { usage_percent: 42.5, core_count: 8, per_core: [] },
		memory: {
			total_bytes: 16 * 1024 ** 3,
			used_bytes: 8 * 1024 ** 3,
			available_bytes: 8 * 1024 ** 3,
			usage_percent: 50,
		},
		disks: [
			{
				name: "disk0",
				mount_point: "/",
				total_bytes: 500 * 1024 ** 3,
				available_bytes: 200 * 1024 ** 3,
				used_bytes: 300 * 1024 ** 3,
				usage_percent: 60,
			},
		],
		load_average: { one: 1.5, five: 1.2, fifteen: 0.9 },
	};
	return {
		history: [snapshot],
		latest: snapshot,
		alerts: [],
		status: "healthy",
		available: true,
		loading: false,
		refetch: vi.fn(),
		...overrides,
	};
}

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
	beforeEach(() => {
		mockUseSystemMetrics.mockReturnValue(systemMetricsResult());
	});

	it("renders the page heading", () => {
		renderPage();
		expect(screen.getByText("Monitoring")).toBeInTheDocument();
	});

	it("renders the system resources section with live data", () => {
		renderPage();
		expect(screen.getByText("CPU Usage")).toBeInTheDocument();
		expect(screen.getByText("Memory Usage")).toBeInTheDocument();
		expect(screen.getByText("Disk Usage")).toBeInTheDocument();
		expect(screen.getByText("Load Average")).toBeInTheDocument();
		// Disk row from the latest snapshot
		expect(screen.getByText("60%")).toBeInTheDocument();
		// No placeholder remnants
		expect(screen.queryByText(/coming soon/i)).toBeNull();
		expect(screen.queryByText(/port 17003/i)).toBeNull();
	});

	it("flags the section when the monitor service is unavailable", () => {
		mockUseSystemMetrics.mockReturnValue(
			systemMetricsResult({
				history: [],
				latest: null,
				available: false,
			}),
		);
		renderPage();
		expect(screen.getByText("monitor service unavailable")).toBeInTheDocument();
		expect(
			screen.getAllByText("Monitor service unavailable").length,
		).toBeGreaterThan(0);
	});

	it("renders refresh interval options", () => {
		renderPage();
		// Scoped to buttons: "1m"/"5m" also appear as Load Average readouts.
		expect(screen.getByRole("button", { name: "10s" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "30s" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "1m" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "5m" })).toBeInTheDocument();
	});

	it("renders a manual refresh button", () => {
		renderPage();
		expect(
			screen.getByRole("button", { name: /refresh metrics/i }),
		).toBeInTheDocument();
	});
});
