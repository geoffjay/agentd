/**
 * MonitoringDashboard — main monitoring page that assembles all monitoring
 * components into a cohesive dashboard.
 *
 * Layout:
 * - Header with refresh controls and last-refresh timestamp
 * - Service metrics cards (one per service: orchestrator, notify, ask)
 * - Agent activity chart + notification metrics chart (side-by-side)
 * - System health panel
 * - Placeholder charts for future monitor service (cpu, memory, disk, network)
 */

import { Clock, RefreshCw } from "lucide-react";
import { AgentActivityChart } from "@/components/monitoring/AgentActivityChart";
import { CacheEfficiencyChart } from "@/components/monitoring/CacheEfficiencyChart";
import { CostOverviewChart } from "@/components/monitoring/CostOverviewChart";
import { NotificationMetricsChart } from "@/components/monitoring/NotificationMetricsChart";
import { PlaceholderChart } from "@/components/monitoring/PlaceholderChart";
import { ServiceMetricsCard } from "@/components/monitoring/ServiceMetricsCard";
import { SystemHealthPanel } from "@/components/monitoring/SystemHealthPanel";
import { TokenUsageChart } from "@/components/monitoring/TokenUsageChart";
import type { RefreshInterval } from "@/hooks/useMetrics";
import { useMetrics } from "@/hooks/useMetrics";
import { useUsageMetrics } from "@/hooks/useUsageMetrics";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const REFRESH_OPTIONS: Array<{ label: string; value: RefreshInterval }> = [
	{ label: "10s", value: 10_000 },
	{ label: "30s", value: 30_000 },
	{ label: "1m", value: 60_000 },
	{ label: "5m", value: 300_000 },
];

function formatRefreshTime(date: Date): string {
	return date.toLocaleTimeString([], {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
	});
}

// ---------------------------------------------------------------------------
// MonitoringDashboard
// ---------------------------------------------------------------------------

export function MonitoringDashboard() {
	const {
		agentCounts,
		notifCounts,
		serviceMetrics,
		agentTimeSeries,
		loading,
		lastRefresh,
		error,
		refetch,
		refreshInterval,
		setRefreshInterval,
	} = useMetrics(30_000);

	const {
		entries: usageEntries,
		aggregate: usageAggregate,
		loading: usageLoading,
	} = useUsageMetrics(refreshInterval);

	// Map response times from serviceMetrics — they come from Prometheus
	// latency data when available, otherwise undefined (SystemHealthPanel
	// measures separately via /health).
	const responseTimeMap: Record<string, number | undefined> = {};
	for (const svc of serviceMetrics) {
		responseTimeMap[svc.key] = svc.http.latencyP50;
	}

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-start justify-between gap-4 flex-wrap">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">Monitoring</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						Real-time agent and service metrics.
					</p>
				</div>

				<div className="flex items-center gap-3">
					{/* Last refresh indicator */}
					{lastRefresh && (
						<div className="flex items-center gap-1.5 text-xs text-th-text-faint">
							<Clock size={12} />
							<span>Updated {formatRefreshTime(lastRefresh)}</span>
						</div>
					)}

					{/* Interval picker */}
					<div
						className="flex items-center rounded-md border border-th-border overflow-hidden text-xs"
						role="group"
						aria-label="Refresh interval"
					>
						{REFRESH_OPTIONS.map((opt) => (
							<button
								key={opt.value}
								type="button"
								onClick={() => setRefreshInterval(opt.value)}
								aria-pressed={refreshInterval === opt.value}
								className={[
									"px-2.5 py-1 transition-colors",
									refreshInterval === opt.value
										? "bg-th-accent-subtle text-th-text-link font-medium"
										: "text-th-text-muted hover:text-th-text-secondary",
								].join(" ")}
							>
								{opt.label}
							</button>
						))}
					</div>

					{/* Manual refresh */}
					<button
						type="button"
						onClick={refetch}
						disabled={loading}
						aria-label="Refresh metrics"
						className="flex items-center gap-1.5 rounded-md border border-th-border px-3 py-1.5 text-xs text-th-text-muted hover:bg-th-surface-hover transition-colors disabled:opacity-50"
					>
						<RefreshCw size={12} className={loading ? "animate-spin" : ""} />
						Refresh
					</button>
				</div>
			</div>

			{/* Error banner */}
			{error && (
				<div className="rounded-md border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					Failed to fetch metrics: {error}
				</div>
			)}

			{/* Service metrics cards */}
			<section aria-label="Service metrics">
				<h2 className="sr-only">Service Metrics</h2>
				<div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
					{loading
						? [0, 1, 2].map((i) => (
								<ServiceMetricsCard
									key={i}
									data={{
										key: "",
										name: "",
										port: 0,
										http: { requestsTotal: 0, errorsTotal: 0, errorRate: 0 },
										reachable: false,
									}}
									loading
								/>
							))
						: serviceMetrics.map((svc) => (
								<ServiceMetricsCard
									key={svc.key}
									data={svc}
									responseTimeMs={responseTimeMap[svc.key]}
								/>
							))}
				</div>
			</section>

			{/* Charts row: agent activity + notification breakdown */}
			<section aria-label="Activity charts">
				<h2 className="sr-only">Activity Charts</h2>
				<div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
					<AgentActivityChart
						counts={agentCounts}
						timeSeries={agentTimeSeries}
						loading={loading}
					/>
					<NotificationMetricsChart counts={notifCounts} loading={loading} />
				</div>
			</section>

			{/* System health grid */}
			<section aria-label="System health">
				<h2 className="sr-only">System Health</h2>
				<SystemHealthPanel serviceMetrics={serviceMetrics} loading={loading} />
			</section>

			{/* Token Usage & Prompt Cache section */}
			<section aria-label="Token usage and prompt cache metrics">
				<h2 className="mb-3 text-sm font-semibold text-th-text-secondary">
					Token Usage &amp; Prompt Cache
				</h2>
				<div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
					<TokenUsageChart entries={usageEntries} loading={usageLoading} />
					<CacheEfficiencyChart
						entries={usageEntries}
						aggregate={usageAggregate}
						loading={usageLoading}
					/>
				</div>
				<div className="mt-4">
					<CostOverviewChart
						entries={usageEntries}
						aggregate={usageAggregate}
						loading={usageLoading}
					/>
				</div>
			</section>

			{/* Placeholder charts for future monitor service */}
			<section aria-label="System resource charts (coming soon)">
				<h2 className="mb-3 text-sm font-semibold text-th-text-secondary">
					System Resources
					<span className="ml-2 text-xs font-normal text-th-text-faint">
						— requires monitor service (port 17003)
					</span>
				</h2>
				<div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
					<PlaceholderChart
						variant="cpu"
						title="CPU Usage"
						description="User, system, and idle time"
					/>
					<PlaceholderChart
						variant="memory"
						title="Memory Usage"
						description="Used, cached, and free"
					/>
					<PlaceholderChart
						variant="disk"
						title="Disk Usage"
						description="Per-mount utilisation"
					/>
					<PlaceholderChart
						variant="network"
						title="Network I/O"
						description="Inbound and outbound traffic"
					/>
				</div>
			</section>
		</div>
	);
}

export default MonitoringDashboard;
