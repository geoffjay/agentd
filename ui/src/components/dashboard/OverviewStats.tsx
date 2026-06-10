/**
 * OverviewStats — compact stat-card row at the top of the dashboard.
 *
 * Each stat shows "—" when its backing service is unreachable (null value)
 * so a single dead service never breaks the rest of the row.
 */

import {
	Bell,
	DollarSign,
	HelpCircle,
	Play,
	ShieldCheck,
	Workflow,
} from "lucide-react";
import { Skeleton } from "@/components/common/LoadingSkeleton";

export interface OverviewStatsProps {
	runningAgents: number | null;
	pendingApprovals: number | null;
	pendingNotifications: number | null;
	pendingQuestions: number | null;
	workflows: number | null;
	totalCostUsd: number | null;
	loading?: boolean;
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

function formatCount(value: number | null): string {
	return value === null ? "—" : String(value);
}

function formatCost(value: number | null): string {
	if (value === null) return "—";
	if (value > 0 && value < 0.01) return "<$0.01";
	return `$${value.toFixed(2)}`;
}

// ---------------------------------------------------------------------------
// Stat card
// ---------------------------------------------------------------------------

interface StatCardProps {
	icon: React.ReactNode;
	label: string;
	value: string;
	loading?: boolean;
}

function StatCard({ icon, label, value, loading }: StatCardProps) {
	return (
		<div className="rounded-lg border border-th-border bg-th-surface px-4 py-3">
			<div className="flex items-center gap-1.5 text-xs text-th-text-faint">
				{icon}
				{label}
			</div>
			{loading ? (
				<Skeleton className="mt-1.5 h-7 w-12" />
			) : (
				<p className="mt-1 text-2xl font-semibold text-th-text">{value}</p>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// OverviewStats
// ---------------------------------------------------------------------------

export function OverviewStats({
	runningAgents,
	pendingApprovals,
	pendingNotifications,
	pendingQuestions,
	workflows,
	totalCostUsd,
	loading = false,
}: OverviewStatsProps) {
	const stats: StatCardProps[] = [
		{
			icon: <Play size={12} />,
			label: "Running Agents",
			value: formatCount(runningAgents),
		},
		{
			icon: <ShieldCheck size={12} />,
			label: "Pending Approvals",
			value: formatCount(pendingApprovals),
		},
		{
			icon: <Bell size={12} />,
			label: "Pending Notifications",
			value: formatCount(pendingNotifications),
		},
		{
			icon: <HelpCircle size={12} />,
			label: "Pending Questions",
			value: formatCount(pendingQuestions),
		},
		{
			icon: <Workflow size={12} />,
			label: "Workflows",
			value: formatCount(workflows),
		},
		{
			icon: <DollarSign size={12} />,
			label: "Total Cost",
			value: formatCost(totalCostUsd),
		},
	];

	return (
		<section aria-label="Overview statistics">
			<div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">
				{stats.map((stat) => (
					<StatCard key={stat.label} {...stat} loading={loading} />
				))}
			</div>
		</section>
	);
}

export default OverviewStats;
