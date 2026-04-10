/**
 * DashboardPage — landing page with service health, agent summary,
 * notification summary, activity timeline, and stub sections.
 */

import { BarChart2, RefreshCw, Webhook } from "lucide-react";
import { useMemo, useState } from "react";
import type { ActivityEvent } from "@/components/dashboard/ActivityTimeline";
import { ActivityTimeline } from "@/components/dashboard/ActivityTimeline";
import { AgentSummary } from "@/components/dashboard/AgentSummary";
import { NotificationSummary } from "@/components/dashboard/NotificationSummary";
import {
	ServiceHealthCard,
	ServiceHealthCardSkeleton,
} from "@/components/dashboard/ServiceHealthCard";
import { useAgentSummary } from "@/hooks/useAgentSummary";
import { useNotificationSummary } from "@/hooks/useNotificationSummary";
import { useServiceHealth } from "@/hooks/useServiceHealth";
import { CreateAgentDialog } from "@/pages/agents/CreateAgentDialog";
import { orchestratorClient } from "@/services/orchestrator";

// ---------------------------------------------------------------------------
// Stub "Coming Soon" card
// ---------------------------------------------------------------------------

interface ComingSoonCardProps {
	title: string;
	icon: React.ReactNode;
}

function ComingSoonCard({ title, icon }: ComingSoonCardProps) {
	return (
		<div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-th-border-strong bg-th-surface p-8 text-center">
			<div className="flex h-12 w-12 items-center justify-center rounded-full bg-th-surface-sunken">
				{icon}
			</div>
			<div>
				<p className="font-medium text-th-text-secondary">{title}</p>
				<p className="mt-1 text-sm text-th-text-faint">
					Coming Soon
				</p>
			</div>
		</div>
	);
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
	const [createOpen, setCreateOpen] = useState(false);

	// Build a synthetic activity feed from available data
	const activityEvents: ActivityEvent[] = useMemo(() => {
		const events: ActivityEvent[] = [];

		// Derive events from recently-updated agents
		for (const agent of agentSummary.recentAgents) {
			events.push({
				id: `agent-${agent.id}`,
				type: "agent",
				description: `Agent "${agent.name}" is ${agent.status.toLowerCase()}`,
				timestamp: new Date(agent.updated_at),
			});
		}

		// Sort by timestamp descending
		return events
			.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime())
			.slice(0, 10);
	}, [agentSummary.recentAgents]);

	return (
		<div className="space-y-6">
			{/* Page header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">
						Dashboard
					</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						System overview and service health
					</p>
				</div>
				<button
					type="button"
					onClick={refresh}
					disabled={healthLoading}
					aria-label="Refresh service health"
					className="flex items-center gap-1.5 rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50"
				>
					<RefreshCw
						size={14}
						className={healthLoading ? "animate-spin" : ""}
					/>
					Refresh
				</button>
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

			{/* Main grid: agents + notifications */}
			<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
				<AgentSummary
					{...agentSummary}
					onCreateAgent={() => setCreateOpen(true)}
				/>
				<NotificationSummary {...notifSummary} />
			</div>

			{/* Activity timeline */}
			<ActivityTimeline
				events={activityEvents}
				loading={agentSummary.loading}
				error={agentSummary.error}
			/>

			{/* Stub sections */}
			<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
				<ComingSoonCard
					title="Monitoring"
					icon={<BarChart2 size={24} className="text-th-text-muted" />}
				/>
				<ComingSoonCard
					title="Hooks"
					icon={<Webhook size={24} className="text-th-text-muted" />}
				/>
			</div>

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
