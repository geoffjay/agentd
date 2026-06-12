/**
 * Tests for ResourceTrendCard component.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TrendSeries } from "@/components/monitoring/ResourceTrendCard";
import { ResourceTrendCard } from "@/components/monitoring/ResourceTrendCard";

// Mock useNivoTheme to avoid ThemeProvider dependency in unit tests
vi.mock("@/hooks/useNivoTheme", () => ({ useNivoTheme: () => ({}) }));

// Mock Nivo line chart
vi.mock("@nivo/line", () => ({
	ResponsiveLine: () => <div role="img" aria-label="trend line chart" />,
}));

const SERIES: TrendSeries[] = [
	{
		id: "CPU",
		color: "#3b82f6",
		data: [
			{ x: new Date("2024-01-01T00:00:00Z"), y: 40 },
			{ x: new Date("2024-01-01T00:00:30Z"), y: 45 },
		],
	},
];

const READOUTS = [{ label: "CPU", value: "45%", color: "#3b82f6" }];

describe("ResourceTrendCard", () => {
	it("renders title, description, and latest readout", () => {
		render(
			<ResourceTrendCard
				title="CPU Usage"
				description="8 cores"
				series={SERIES}
				readouts={READOUTS}
				yMax={100}
				unit="%"
			/>,
		);
		expect(screen.getByText("CPU Usage")).toBeTruthy();
		expect(screen.getByText("8 cores")).toBeTruthy();
		expect(screen.getByText("45%")).toBeTruthy();
		expect(screen.getByLabelText("trend line chart")).toBeTruthy();
	});

	it("shows a loading skeleton while loading", () => {
		const { container } = render(
			<ResourceTrendCard
				title="CPU Usage"
				series={SERIES}
				readouts={READOUTS}
				loading
			/>,
		);
		expect(container.querySelector(".animate-pulse")).toBeTruthy();
		expect(screen.queryByLabelText("trend line chart")).toBeNull();
	});

	it("shows the unavailable state when the monitor is down", () => {
		render(
			<ResourceTrendCard
				title="CPU Usage"
				series={[]}
				readouts={[]}
				available={false}
			/>,
		);
		expect(screen.getByText("Monitor service unavailable")).toBeTruthy();
		expect(screen.queryByLabelText("trend line chart")).toBeNull();
	});

	it("shows the empty state when available but no snapshots yet", () => {
		render(
			<ResourceTrendCard
				title="CPU Usage"
				series={[{ id: "CPU", color: "#000", data: [] }]}
				readouts={[]}
			/>,
		);
		expect(screen.getByText("No metrics collected yet.")).toBeTruthy();
	});
});
