/**
 * DashboardPage — landing page with overview stats, system metrics,
 * activity charts, agent/notification summaries, service health, and a
 * recent activity feed.
 */

import { RefreshCw } from "lucide-react";
import { useMemo, useState } from "react";
import { ActivityChart } from "@/components/dashboard/ActivityChart";
import { ActivityTimeline } from "@/components/dashboard/ActivityTimeline";
import { AgentSummary } from "@/components/dashboard/AgentSummary";
import { NotificationSummary } from "@/components/dashboard/NotificationSummary";
import { OverviewStats } from "@/components/dashboard/OverviewStats";
import {
	ServiceHealthCard,
	ServiceHealthCardSkeleton,
} from "@/components/dashboard/ServiceHealthCard";
import { SystemMetricsChart } from "@/components/dashboard/SystemMetricsChart";
import { useActivityFeed } from "@/hooks/useActivityFeed";
import { useAgentSummary } from "@/hooks/useAgentSummary";
import { useDashboardStats } from "@/hooks/useDashboardStats";
import { useNotificationSummary } from "@/hooks/useNotificationSummary";
import { useServiceHealth } from "@/hooks/useServiceHealth";
import { useSystemMetrics } from "@/hooks/useSystemMetrics";
import { CreateAgentDialog } from "@/pages/agents/CreateAgentDialog";
import { orchestratorClient } from "@/services/orchestrator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatRelativeTime(date: Date): string {
	const diffSec = Math.floor((Date.now() - date.getTime()) / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin}m ago`;
	return `${Math.floor(diffMin / 60)}h ago`;
}

// ---------------------------------------------------------------------------
// Dashboard page
// ---------------------------------------------------------------------------

export function DashboardPage() {
	const {
		services,
		loading: healthLoading,
		initializing: healthInit,
		refresh,
	} = useServiceHealth();
	const agentSummary = useAgentSummary();
	const notifSummary = useNotificationSummary();
	const systemMetrics = useSystemMetrics();
	const dashboardStats = useDashboardStats();
	const activityFeed = useActivityFeed();
	const [createOpen, setCreateOpen] = useState(false);

	// Most recent successful health check — doubles as the "Updated" stamp
	// since service health refreshes on the same cadence as everything else.
	const lastUpdated = useMemo(() => {
		let latest: Date | null = null;
		for (const svc of services) {
			if (svc.lastChecked && (!latest || svc.lastChecked > latest)) {
				latest = svc.lastChecked;
			}
		}
		return latest;
	}, [services]);

	function refreshAll() {
		refresh();
		agentSummary.refetch();
		notifSummary.refetch();
		systemMetrics.refetch();
		dashboardStats.refetch();
		activityFeed.refetch();
	}

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">Dashboard</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						System overview and service health
					</p>
				</div>
				<div className="flex items-center gap-3">
					{lastUpdated && (
						<span className="text-xs text-th-text-faint">
							Updated {formatRelativeTime(lastUpdated)}
						</span>
					)}
					<button
						type="button"
						onClick={refreshAll}
						disabled={healthLoading}
						aria-label="Refresh dashboard"
						className="flex items-center gap-1.5 rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
					>
						<RefreshCw
							size={14}
							className={healthLoading ? "animate-spin" : ""}
						/>
						Refresh
					</button>
				</div>
			</div>

			{/* Overview stat row */}
			<OverviewStats
				runningAgents={agentSummary.error ? null : agentSummary.counts.running}
				pendingApprovals={dashboardStats.pendingApprovals}
				pendingNotifications={notifSummary.error ? null : notifSummary.pending}
				pendingQuestions={dashboardStats.pendingQuestions}
				workflows={dashboardStats.workflows}
				totalCostUsd={agentSummary.aggregateUsage?.totalCostUsd ?? null}
				loading={
					agentSummary.loading || notifSummary.loading || dashboardStats.loading
				}
			/>

			{/* Time-series charts */}
			<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
				<SystemMetricsChart {...systemMetrics} />
				<ActivityChart
					buckets={activityFeed.buckets}
					loading={activityFeed.loading}
					error={activityFeed.error}
				/>
			</div>

			{/* Main grid: agents + notifications */}
			<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
				<AgentSummary
					{...agentSummary}
					onCreateAgent={() => setCreateOpen(true)}
				/>
				<NotificationSummary {...notifSummary} />
			</div>

			{/* Service health cards */}
			<section aria-labelledby="service-health-heading">
				<h2
					id="service-health-heading"
					className="mb-3 text-sm font-medium uppercase tracking-wide text-th-text-faint"
				>
					Service Health
				</h2>
				<div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
					{healthInit
						? Array.from({ length: 6 }).map((_, i) => (
								<ServiceHealthCardSkeleton key={i} />
							))
						: services.map((svc) => (
								<ServiceHealthCard key={svc.key} service={svc} />
							))}
				</div>
			</section>

			{/* Activity timeline */}
			<ActivityTimeline
				events={activityFeed.events}
				loading={activityFeed.loading}
				error={activityFeed.error}
			/>

			{/* Create agent dialog */}
			<CreateAgentDialog
				open={createOpen}
				onClose={() => setCreateOpen(false)}
				onCreate={async (request) => {
					await orchestratorClient.createAgent(request);
					agentSummary.refetch();
				}}
			/>
		</div>
	);
}

export default DashboardPage;
