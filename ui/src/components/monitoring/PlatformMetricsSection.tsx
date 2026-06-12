/**
 * PlatformMetricsSection — Prometheus-backed platform health from the
 * monitor service's curated named-query API.
 *
 * Stat cards (dispatch success, agents vs connections, approvals backlog,
 * session cost, restarts) over per-service HTTP error-rate and p95-latency
 * bar lists. Renders explicit degraded states that distinguish "Prometheus
 * offline" (system resources still live) from "monitor unreachable".
 */

import { AlertTriangle } from "lucide-react";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import type {
	PlatformQueryResults,
	UsePlatformMetricsResult,
} from "@/hooks/usePlatformMetrics";
import { vectorEntries, vectorValue } from "@/hooks/usePlatformMetrics";

// ---------------------------------------------------------------------------
// Stat card
// ---------------------------------------------------------------------------

export type StatTone = "neutral" | "ok" | "warn" | "error";

export interface PlatformStatCardProps {
	label: string;
	/** Pre-formatted display value; "—" when no data */
	value: string;
	/** Small line under the value (e.g. the window or a breakdown) */
	hint?: string;
	tone?: StatTone;
}

const TONE_CLASSES: Record<StatTone, string> = {
	neutral: "text-th-text",
	ok: "text-th-status-success-text",
	warn: "text-th-status-warning-text",
	error: "text-th-status-error-text",
};

export function PlatformStatCard({
	label,
	value,
	hint,
	tone = "neutral",
}: PlatformStatCardProps) {
	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-4">
			<p className="text-xs text-th-text-muted">{label}</p>
			<p className={`mt-1 text-2xl font-semibold ${TONE_CLASSES[tone]}`}>
				{value}
			</p>
			{hint && <p className="mt-0.5 text-xs text-th-text-faint">{hint}</p>}
		</div>
	);
}

// ---------------------------------------------------------------------------
// Per-service bar list
// ---------------------------------------------------------------------------

export interface ServiceBarListProps {
	title: string;
	/** Window hint shown next to the title (e.g. "15m") */
	window?: string;
	entries: Array<{ service: string; value: number }>;
	/** Format a value for the row readout */
	format: (value: number) => string;
	/** Bar fill fraction for a value (clamped to [0, 1]) */
	fraction: (value: number) => number;
	/** Threshold above which a row renders in the warning colour */
	warnAt: number;
	emptyText: string;
}

export function ServiceBarList({
	title,
	window: windowHint,
	entries,
	format,
	fraction,
	warnAt,
	emptyText,
}: ServiceBarListProps) {
	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-4">
			<h3 className="text-sm font-semibold text-th-text">
				{title}
				{windowHint && (
					<span className="ml-2 text-xs font-normal text-th-text-faint">
						{windowHint}
					</span>
				)}
			</h3>
			{entries.length === 0 ? (
				<p className="mt-3 text-xs text-th-text-muted">{emptyText}</p>
			) : (
				<ul className="mt-3 space-y-2">
					{entries.map((entry) => {
						const warn = entry.value >= warnAt;
						return (
							<li
								key={entry.service}
								className="flex items-center gap-3 text-xs"
							>
								<span className="w-24 shrink-0 truncate text-th-text-muted">
									{entry.service}
								</span>
								<div className="h-1.5 flex-1 overflow-hidden rounded-full bg-th-surface-hover">
									<div
										className="h-full rounded-full"
										style={{
											width: `${Math.min(100, fraction(entry.value) * 100)}%`,
											background: warn
												? "var(--th-status-warning-text, #f59e0b)"
												: "#3b82f6",
										}}
									/>
								</div>
								<span
									className={`w-16 shrink-0 text-right font-medium ${
										warn ? "text-th-status-warning-text" : "text-th-text"
									}`}
								>
									{format(entry.value)}
								</span>
							</li>
						);
					})}
				</ul>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// Derivations
// ---------------------------------------------------------------------------

function dispatchSuccessCard(results: PlatformQueryResults) {
	const rate = vectorValue(results["dispatch-success-rate"]);
	const throughput = vectorEntries(results["dispatch-throughput"]);
	const byStatus = Object.fromEntries(
		throughput.map((e) => [e.labels.status ?? "?", Math.round(e.value)]),
	);
	const finished = (byStatus.completed ?? 0) + (byStatus.failed ?? 0);

	const hint =
		throughput.length > 0
			? `${byStatus.completed ?? 0} completed / ${byStatus.failed ?? 0} failed (1h)`
			: "last 1h";

	if (rate === null || finished === 0) {
		return { value: "—", hint: "no dispatches in the window", tone: "neutral" as StatTone };
	}
	const tone: StatTone = rate >= 0.9 ? "ok" : rate >= 0.5 ? "warn" : "error";
	return { value: `${(rate * 100).toFixed(0)}%`, hint, tone };
}

function agentsCard(results: PlatformQueryResults) {
	const active = vectorValue(results["agents-active"]);
	const connections = vectorValue(results["websocket-connections"]);
	if (active === null) {
		return { value: "—", hint: "agents active", tone: "neutral" as StatTone };
	}
	const mismatch = connections !== null && connections !== active;
	return {
		value: `${active}`,
		hint:
			connections === null
				? "agents active"
				: `${connections} connection${connections === 1 ? "" : "s"}${mismatch ? " — state mismatch?" : ""}`,
		tone: (mismatch ? "warn" : "neutral") as StatTone,
	};
}

// ---------------------------------------------------------------------------
// Section
// ---------------------------------------------------------------------------

export type PlatformMetricsSectionProps = UsePlatformMetricsResult;

export function PlatformMetricsSection({
	results,
	monitorDown,
	prometheusDown,
	loading,
}: PlatformMetricsSectionProps) {
	const dispatch = dispatchSuccessCard(results);
	const agents = agentsCard(results);
	const approvals = vectorValue(results["approvals-backlog"]);
	const cost = vectorValue(results["session-cost"]);
	const restarts = vectorValue(results["agent-restart-rate"]);

	const errorRates = vectorEntries(results["http-error-rate"]).map((e) => ({
		service: e.labels.service ?? "?",
		value: e.value,
	}));
	const latencies = vectorEntries(results["http-p95-latency"])
		.map((e) => ({
			service: e.labels.service ?? "?",
			// histogram_quantile yields NaN for services with no traffic yet.
			value: e.value,
		}))
		.filter((e) => Number.isFinite(e.value));

	const degraded = monitorDown || prometheusDown;

	return (
		<section aria-label="Platform metrics">
			<h2 className="mb-3 text-sm font-semibold text-th-text-secondary">
				Platform Metrics
			</h2>

			{/* Degraded banner */}
			{!loading && degraded && (
				<div className="mb-4 flex items-center gap-2 rounded-md border border-th-status-warning-border bg-th-status-warning-bg px-4 py-2.5 text-sm text-th-status-warning-text">
					<AlertTriangle size={14} className="shrink-0" />
					{monitorDown
						? "Monitor service unreachable — platform metrics unavailable."
						: "Prometheus is offline — platform metrics unavailable (system resources are still live)."}
				</div>
			)}

			{loading && <ChartSkeleton height={96} />}

			{!loading && !degraded && (
				<>
					<div className="grid grid-cols-2 gap-4 sm:grid-cols-3 xl:grid-cols-5">
						<PlatformStatCard
							label="Dispatch Success"
							value={dispatch.value}
							hint={dispatch.hint}
							tone={dispatch.tone}
						/>
						<PlatformStatCard
							label="Active Agents"
							value={agents.value}
							hint={agents.hint}
							tone={agents.tone}
						/>
						<PlatformStatCard
							label="Approvals Backlog"
							value={approvals === null ? "—" : `${Math.round(approvals)}`}
							hint="pending tool approvals"
							tone={approvals !== null && approvals > 0 ? "warn" : "neutral"}
						/>
						<PlatformStatCard
							label="Session Cost"
							value={cost === null ? "—" : `$${cost.toFixed(2)}`}
							hint="last 24h"
						/>
						<PlatformStatCard
							label="Agent Restarts"
							value={restarts === null ? "—" : `${Math.round(restarts)}`}
							hint="last 1h"
							tone={restarts !== null && restarts > 0 ? "warn" : "neutral"}
						/>
					</div>

					<div className="mt-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
						<ServiceBarList
							title="HTTP Error Rate"
							window="15m"
							entries={errorRates}
							format={(v) => `${(v * 100).toFixed(2)}%`}
							fraction={(v) => v}
							warnAt={0.01}
							emptyText="No HTTP traffic in the window."
						/>
						<ServiceBarList
							title="HTTP p95 Latency"
							window="15m"
							entries={latencies}
							format={(v) => `${(v * 1000).toFixed(0)} ms`}
							fraction={(v) => v / 1}
							warnAt={0.5}
							emptyText="No latency samples in the window."
						/>
					</div>
				</>
			)}
		</section>
	);
}

export default PlatformMetricsSection;
