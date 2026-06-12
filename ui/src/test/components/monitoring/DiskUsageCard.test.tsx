/**
 * Tests for DiskUsageCard component.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DiskUsageCard } from "@/components/monitoring/DiskUsageCard";
import type { DiskMetrics } from "@/types/monitor";

const GIB = 1024 ** 3;

const DISKS: DiskMetrics[] = [
	{
		name: "disk0",
		mount_point: "/",
		total_bytes: 500 * GIB,
		available_bytes: 200 * GIB,
		used_bytes: 300 * GIB,
		usage_percent: 60,
	},
	{
		name: "disk1",
		mount_point: "/data",
		total_bytes: 1000 * GIB,
		available_bytes: 50 * GIB,
		used_bytes: 950 * GIB,
		usage_percent: 95,
	},
];

describe("DiskUsageCard", () => {
	it("renders one row per mount point with usage", () => {
		render(<DiskUsageCard disks={DISKS} />);
		expect(screen.getByText("/")).toBeTruthy();
		expect(screen.getByText("/data")).toBeTruthy();
		expect(screen.getByText("60%")).toBeTruthy();
		expect(screen.getByText("95%")).toBeTruthy();
	});

	it("renders accessible progress bars with the usage value", () => {
		render(<DiskUsageCard disks={DISKS} />);
		const bars = screen.getAllByRole("progressbar");
		expect(bars).toHaveLength(2);
		expect(bars[0].getAttribute("aria-valuenow")).toBe("60");
		expect(bars[1].getAttribute("aria-valuenow")).toBe("95");
	});

	it("formats sizes in GiB", () => {
		render(<DiskUsageCard disks={[DISKS[0]]} />);
		expect(screen.getByText(/300 GiB/)).toBeTruthy();
		expect(screen.getByText(/500 GiB/)).toBeTruthy();
	});

	it("shows the unavailable state when the monitor is down", () => {
		render(<DiskUsageCard disks={[]} available={false} />);
		expect(screen.getByText("Monitor service unavailable")).toBeTruthy();
		expect(screen.queryAllByRole("progressbar")).toHaveLength(0);
	});

	it("shows a loading skeleton while loading", () => {
		const { container } = render(<DiskUsageCard disks={[]} loading />);
		expect(container.querySelector(".animate-pulse")).toBeTruthy();
	});
});
