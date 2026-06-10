/**
 * OverviewStats -- rendering tests, including the "—" fallback for
 * unavailable (null) data sources.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { OverviewStatsProps } from "@/components/dashboard/OverviewStats";
import { OverviewStats } from "@/components/dashboard/OverviewStats";

const baseProps: OverviewStatsProps = {
	runningAgents: 3,
	pendingApprovals: 2,
	pendingNotifications: 5,
	pendingQuestions: 1,
	workflows: 4,
	totalCostUsd: 1.23,
	loading: false,
};

describe("OverviewStats", () => {
	it("renders all six stat labels", () => {
		render(<OverviewStats {...baseProps} />);
		expect(screen.getByText("Running Agents")).toBeInTheDocument();
		expect(screen.getByText("Pending Approvals")).toBeInTheDocument();
		expect(screen.getByText("Pending Notifications")).toBeInTheDocument();
		expect(screen.getByText("Pending Questions")).toBeInTheDocument();
		expect(screen.getByText("Workflows")).toBeInTheDocument();
		expect(screen.getByText("Total Cost")).toBeInTheDocument();
	});

	it("renders numeric values", () => {
		render(<OverviewStats {...baseProps} />);
		expect(screen.getByText("3")).toBeInTheDocument();
		expect(screen.getByText("5")).toBeInTheDocument();
	});

	it("formats the total cost as dollars", () => {
		render(<OverviewStats {...baseProps} />);
		expect(screen.getByText("$1.23")).toBeInTheDocument();
	});

	it("formats tiny non-zero costs as <$0.01", () => {
		render(<OverviewStats {...baseProps} totalCostUsd={0.001} />);
		expect(screen.getByText("<$0.01")).toBeInTheDocument();
	});

	it('renders "—" when a source is unavailable (null)', () => {
		render(
			<OverviewStats
				{...baseProps}
				pendingApprovals={null}
				totalCostUsd={null}
			/>,
		);
		expect(screen.getAllByText("—")).toHaveLength(2);
	});

	it('renders "—" for every stat when all sources are unavailable', () => {
		render(
			<OverviewStats
				runningAgents={null}
				pendingApprovals={null}
				pendingNotifications={null}
				pendingQuestions={null}
				workflows={null}
				totalCostUsd={null}
			/>,
		);
		expect(screen.getAllByText("—")).toHaveLength(6);
	});

	it("renders skeletons instead of values while loading", () => {
		render(<OverviewStats {...baseProps} loading />);
		expect(screen.queryByText("3")).not.toBeInTheDocument();
		expect(screen.queryByText("$1.23")).not.toBeInTheDocument();
	});
});
