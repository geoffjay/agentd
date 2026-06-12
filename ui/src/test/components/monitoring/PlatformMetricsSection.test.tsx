/**
 * Tests for PlatformMetricsSection and its building blocks.
 */

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	PlatformMetricsSection,
	PlatformStatCard,
	ServiceBarList,
} from "@/components/monitoring/PlatformMetricsSection";
import type { PlatformQueryResults } from "@/hooks/usePlatformMetrics";
import {
	makeVectorQueryResult,
	makeVectorSample,
} from "@/test/mocks/factories";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function vector(name: string, samples: Array<[Record<string, string>, string]>) {
	return makeVectorQueryResult(
		name,
		samples.map(([metric, value]) =>
			makeVectorSample({ metric, value: [1, value] }),
		),
	);
}

const HEALTHY_RESULTS: PlatformQueryResults = {
	"dispatch-success-rate": vector("dispatch-success-rate", [[{}, "0.95"]]),
	"dispatch-throughput": vector("dispatch-throughput", [
		[{ status: "completed" }, "19"],
		[{ status: "failed" }, "1"],
	]),
	"agent-restart-rate": vector("agent-restart-rate", [[{}, "0"]]),
	"approvals-backlog": vector("approvals-backlog", [[{}, "0"]]),
	"http-error-rate": vector("http-error-rate", [
		[{ service: "orchestrator" }, "0.001"],
	]),
	"http-p95-latency": vector("http-p95-latency", [
		[{ service: "orchestrator" }, "0.0125"],
		[{ service: "notify" }, "NaN"],
	]),
	"session-cost": vector("session-cost", [[{}, "1.5"]]),
	"agents-active": vector("agents-active", [[{}, "3"]]),
	"websocket-connections": vector("websocket-connections", [[{}, "3"]]),
};

function renderSection(overrides?: Record<string, unknown>) {
	return render(
		<PlatformMetricsSection
			results={HEALTHY_RESULTS}
			monitorDown={false}
			prometheusDown={false}
			loading={false}
			refetch={vi.fn()}
			{...overrides}
		/>,
	);
}

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

describe("PlatformMetricsSection", () => {
	it("renders stat cards from the query results", () => {
		renderSection();
		expect(screen.getByText("Dispatch Success")).toBeTruthy();
		expect(screen.getByText("95%")).toBeTruthy();
		expect(screen.getByText("19 completed / 1 failed (1h)")).toBeTruthy();
		expect(screen.getByText("Active Agents")).toBeTruthy();
		expect(screen.getByText("3 connections")).toBeTruthy();
		expect(screen.getByText("Session Cost")).toBeTruthy();
		expect(screen.getByText("$1.50")).toBeTruthy();
	});

	it("renders per-service bars, converting p95 to ms and dropping NaN rows", () => {
		renderSection();
		expect(screen.getByText("HTTP p95 Latency")).toBeTruthy();
		expect(screen.getByText("13 ms")).toBeTruthy();
		expect(screen.getByText("0.10%")).toBeTruthy();
		// notify's NaN latency row (no traffic) is filtered out:
		// "orchestrator" appears in both bar lists, "notify" in neither.
		expect(screen.getAllByText("orchestrator")).toHaveLength(2);
		expect(screen.queryByText("notify")).toBeNull();
	});

	it("flags an agents/connections mismatch", () => {
		renderSection({
			results: {
				...HEALTHY_RESULTS,
				"websocket-connections": vector("websocket-connections", [[{}, "1"]]),
			},
		});
		expect(screen.getByText(/state mismatch\?/)).toBeTruthy();
	});

	it("shows the Prometheus-offline banner and hides the cards", () => {
		renderSection({ prometheusDown: true, results: {} });
		expect(screen.getByText(/Prometheus is offline/)).toBeTruthy();
		expect(
			screen.getByText(/system resources are still live/),
		).toBeTruthy();
		expect(screen.queryByText("Dispatch Success")).toBeNull();
	});

	it("shows the monitor-unreachable banner when the monitor is down", () => {
		renderSection({ monitorDown: true, results: {} });
		expect(screen.getByText(/Monitor service unreachable/)).toBeTruthy();
	});

	it("renders an em-dash placeholder when a query has no data", () => {
		renderSection({
			results: {
				...HEALTHY_RESULTS,
				"dispatch-success-rate": vector("dispatch-success-rate", []),
				"dispatch-throughput": vector("dispatch-throughput", []),
			},
		});
		expect(screen.getByText("no dispatches in the window")).toBeTruthy();
	});
});

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

describe("PlatformStatCard", () => {
	it("renders label, value, and hint", () => {
		render(
			<PlatformStatCard
				label="Approvals Backlog"
				value="4"
				hint="pending tool approvals"
				tone="warn"
			/>,
		);
		expect(screen.getByText("Approvals Backlog")).toBeTruthy();
		expect(screen.getByText("4")).toBeTruthy();
		expect(screen.getByText("pending tool approvals")).toBeTruthy();
	});
});

describe("ServiceBarList", () => {
	it("renders the empty state when there are no entries", () => {
		render(
			<ServiceBarList
				title="HTTP Error Rate"
				entries={[]}
				format={(v) => `${v}`}
				fraction={(v) => v}
				warnAt={1}
				emptyText="No HTTP traffic in the window."
			/>,
		);
		expect(screen.getByText("No HTTP traffic in the window.")).toBeTruthy();
	});
});
