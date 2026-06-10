/**
 * AgentSummary — shows agent status distribution as a donut chart
 * plus the 5 most recently updated agents.
 */

import { ResponsivePie } from "@nivo/pie";
import { DollarSign, Hash, Layers, Plus, RefreshCw } from "lucide-react";
import { useNavigate } from "react-router-dom";
import {
	ChartSkeleton,
	ListItemSkeleton,
} from "@/components/common/LoadingSkeleton";
import { StatusBadge } from "@/components/common/StatusBadge";
import type { UseAgentSummaryResult } from "@/hooks/useAgentSummary";
import { useChartPalette } from "@/hooks/useChartPalette";
import type { Agent } from "@/types/orchestrator";

/** Alpha used for chart fills — muted so arcs don't overpower the page */
const FILL_ALPHA = 0.7;

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface AgentRowProps {
	agent: Agent;
}

function AgentRow({ agent }: AgentRowProps) {
	const navigate = useNavigate();
	return (
		<li
			className="flex cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 hover:bg-th-surface-hover"
			onClick={() => navigate(`/agents`)}
		>
			<div className="min-w-0">
				<p className="truncate text-sm font-medium text-th-text">
					{agent.name}
				</p>
				<p className="text-xs text-th-text-muted">
					{formatRelativeTime(new Date(agent.updated_at))}
				</p>
			</div>
			<StatusBadge status={agent.status} />
		</li>
	);
}

// ---------------------------------------------------------------------------
// AgentSummary
// ---------------------------------------------------------------------------

interface AgentSummaryProps extends UseAgentSummaryResult {
	onCreateAgent?: () => void;
}

export function AgentSummary({
	counts,
	recentAgents,
	total,
	aggregateUsage,
	loading,
	error,
	onCreateAgent,
}: AgentSummaryProps) {
	const palette = useChartPalette();

	// Theme-aware status colors (full strength — for legend dots only)
	const statusColors: Record<string, string> = {
		running: palette.success,
		pending: palette.warning,
		stopped: palette.neutral,
		failed: palette.error,
	};

	const pieData = [
		{
			id: "Running",
			label: "Running",
			value: counts.running,
			color: statusColors.running,
		},
		{
			id: "Pending",
			label: "Pending",
			value: counts.pending,
			color: statusColors.pending,
		},
		{
			id: "Stopped",
			label: "Stopped",
			value: counts.stopped,
			color: statusColors.stopped,
		},
		{
			id: "Failed",
			label: "Failed",
			value: counts.failed,
			color: statusColors.failed,
		},
	].filter((d) => d.value > 0);

	const hasData = total > 0;

	return (
		<section
			aria-labelledby="agent-summary-heading"
			className="rounded-lg border border-th-border bg-th-surface p-5"
		>
			{/* Header */}
			<div className="flex items-center justify-between">
				<h2
					id="agent-summary-heading"
					className="text-base font-semibold text-th-text"
				>
					Agents
					{!loading && (
						<span className="ml-2 text-sm font-normal text-th-text-muted">
							({total} total)
						</span>
					)}
				</h2>
				<button
					type="button"
					onClick={onCreateAgent}
					aria-label="Create new agent"
					className="flex items-center gap-1.5 rounded-md bg-th-accent px-3 py-1.5 text-xs font-medium text-th-accent-text hover:bg-th-accent-hover focus-visible:outline-none focus-visible:ring-2 focus:ring-th-focus-ring"
				>
					<Plus size={14} />
					Create Agent
				</button>
			</div>

			{/* Error state */}
			{error && (
				<div className="mt-4 flex items-center gap-2 text-sm text-error-500">
					<RefreshCw size={14} />
					{error}
				</div>
			)}

			{/* Loading */}
			{loading && !error && (
				<div className="mt-4">
					<ChartSkeleton height={160} />
					<div className="mt-4">
						<ListItemSkeleton rows={3} />
					</div>
				</div>
			)}

			{/* Content */}
			{!loading && !error && (
				<>
					{/* Donut chart */}
					{hasData ? (
						<div className="relative mt-4 h-40 w-full">
							<ResponsivePie
								data={pieData}
								colors={(d) => palette.withAlpha(d.data.color, FILL_ALPHA)}
								innerRadius={0.6}
								padAngle={2}
								cornerRadius={3}
								borderWidth={1}
								borderColor={palette.surface}
								enableArcLabels={false}
								enableArcLinkLabels={false}
								tooltip={({ datum }) => (
									<div className="rounded bg-th-page-inset px-2 py-1 text-xs text-th-text shadow">
										{datum.label}: {datum.value}
									</div>
								)}
								margin={{ top: 8, right: 8, bottom: 8, left: 8 }}
							/>
						</div>
					) : (
						<p className="mt-4 text-sm text-th-text-muted">No agents yet.</p>
					)}

					{/* Status count pills */}
					{hasData && (
						<div className="mt-3 flex flex-wrap gap-2">
							{Object.entries(counts).map(([status, count]) =>
								count > 0 ? (
									<span
										key={status}
										className="flex items-center gap-1 rounded-full bg-th-surface-sunken px-2.5 py-0.5 text-xs font-medium text-th-text-secondary"
									>
										<span
											className="h-2 w-2 rounded-full"
											style={{ background: statusColors[status] }}
										/>
										{count} {status}
									</span>
								) : null,
							)}
						</div>
					)}

					{/* Aggregate usage stats */}
					{aggregateUsage && (
						<div
							className="mt-3 grid grid-cols-3 gap-2"
							data-testid="aggregate-usage"
						>
							<div className="rounded-md bg-th-surface-sunken px-3 py-2">
								<div className="flex items-center gap-1 text-xs text-th-text-faint">
									<DollarSign size={12} />
									Total Cost
								</div>
								<p className="mt-0.5 text-sm font-semibold text-th-text">
									$
									{aggregateUsage.totalCostUsd < 0.01 &&
									aggregateUsage.totalCostUsd > 0
										? "<0.01"
										: aggregateUsage.totalCostUsd.toFixed(2)}
								</p>
							</div>
							<div className="rounded-md bg-th-surface-sunken px-3 py-2">
								<div className="flex items-center gap-1 text-xs text-th-text-faint">
									<Hash size={12} />
									Tokens
								</div>
								<p className="mt-0.5 text-sm font-semibold text-th-text">
									{aggregateUsage.totalTokens >= 1_000_000
										? `${(aggregateUsage.totalTokens / 1_000_000).toFixed(1)}M`
										: aggregateUsage.totalTokens >= 1_000
											? `${(aggregateUsage.totalTokens / 1_000).toFixed(1)}k`
											: aggregateUsage.totalTokens}
								</p>
							</div>
							<div className="rounded-md bg-th-surface-sunken px-3 py-2">
								<div className="flex items-center gap-1 text-xs text-th-text-faint">
									<Layers size={12} />
									Cache Hit
								</div>
								<p className="mt-0.5 text-sm font-semibold text-th-text">
									{aggregateUsage.cacheHitPercent.toFixed(0)}%
								</p>
							</div>
						</div>
					)}

					{/* Recent agents list */}
					{recentAgents.length > 0 && (
						<div className="mt-4">
							<p className="mb-1 text-xs font-medium uppercase tracking-wide text-th-text-faint">
								Recently Active
							</p>
							<ul role="list" className="space-y-0.5">
								{recentAgents.map((agent) => (
									<AgentRow key={agent.id} agent={agent} />
								))}
							</ul>
						</div>
					)}
				</>
			)}
		</section>
	);
}

export default AgentSummary;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatRelativeTime(date: Date): string {
	const diffMs = Date.now() - date.getTime();
	const diffSec = Math.floor(diffMs / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin} min ago`;
	const diffHr = Math.floor(diffMin / 60);
	if (diffHr < 24) return `${diffHr}h ago`;
	return `${Math.floor(diffHr / 24)}d ago`;
}
