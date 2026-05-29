/**
 * NotificationSummary — shows pending/unread counts and priority breakdown bar chart.
 */

import { ResponsiveBar } from "@nivo/bar";
import { Bell, ExternalLink } from "lucide-react";
import { Link } from "react-router-dom";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import type { UseNotificationSummaryResult } from "@/hooks/useNotificationSummary";

const PRIORITY_COLORS: Record<string, string> = {
	low: "#94a3b8",
	normal: "#60a5fa",
	high: "#f59e0b",
	urgent: "#ef4444",
};

type NotificationSummaryProps = UseNotificationSummaryResult;

export function NotificationSummary({
	pending,
	unread,
	total,
	priorityCounts,
	loading,
	error,
}: NotificationSummaryProps) {
	const barData = [
		{ priority: "low", count: priorityCounts.low, color: PRIORITY_COLORS.low },
		{
			priority: "normal",
			count: priorityCounts.normal,
			color: PRIORITY_COLORS.normal,
		},
		{
			priority: "high",
			count: priorityCounts.high,
			color: PRIORITY_COLORS.high,
		},
		{
			priority: "urgent",
			count: priorityCounts.urgent,
			color: PRIORITY_COLORS.urgent,
		},
	];

	const hasData = total > 0;

	return (
		<section
			aria-labelledby="notification-summary-heading"
			className="rounded-lg border border-th-border bg-th-surface p-5"
		>
			{/* Header */}
			<div className="flex items-center justify-between">
				<h2
					id="notification-summary-heading"
					className="text-base font-semibold text-th-text"
				>
					Notifications
				</h2>
				<Link
					to="/notifications"
					className="flex items-center gap-1 text-xs font-medium text-th-text-link hover:opacity-80"
				>
					View All <ExternalLink size={12} />
				</Link>
			</div>

			{/* Error state */}
			{error && (
				<p className="mt-3 text-sm text-th-status-error-text">{error}</p>
			)}

			{/* Loading */}
			{loading && !error && (
				<div className="mt-4">
					<ChartSkeleton height={120} />
				</div>
			)}

			{/* Content */}
			{!loading && !error && (
				<>
					{/* Summary counts */}
					<div className="mt-4 grid grid-cols-2 gap-3">
						<div className="rounded-md bg-th-status-warning-bg p-3">
							<div className="flex items-center gap-2">
								<Bell size={16} className="text-th-status-warning-text" />
								<span className="text-xs text-th-status-warning-text">
									Pending
								</span>
							</div>
							<p className="mt-1 text-2xl font-bold text-th-status-warning-text">
								{pending}
							</p>
						</div>
						<div className="rounded-md bg-th-status-info-bg p-3">
							<div className="flex items-center gap-2">
								<Bell size={16} className="text-th-status-info-text" />
								<span className="text-xs text-th-status-info-text">Unread</span>
							</div>
							<p className="mt-1 text-2xl font-bold text-th-status-info-text">
								{unread}
							</p>
						</div>
					</div>

					{/* Priority bar chart */}
					{hasData ? (
						<div className="mt-4">
							<p className="mb-2 text-xs font-medium uppercase tracking-wide text-th-text-faint">
								By Priority (active)
							</p>
							<div className="h-28">
								<ResponsiveBar
									data={barData}
									keys={["count"]}
									indexBy="priority"
									colors={({ data }) =>
										PRIORITY_COLORS[data.priority as string] ?? "#94a3b8"
									}
									enableLabel={false}
									axisLeft={null}
									axisBottom={{
										tickSize: 0,
										tickPadding: 6,
									}}
									borderRadius={3}
									padding={0.3}
									margin={{ top: 0, right: 0, bottom: 24, left: 0 }}
									tooltip={({ indexValue, value }) => (
										<div className="rounded bg-th-surface-raised px-2 py-1 text-xs text-th-text shadow">
											{indexValue}: {value}
										</div>
									)}
									theme={{
										axis: {
											ticks: {
												text: { fill: "#94a3b8", fontSize: 11 },
											},
										},
									}}
								/>
							</div>
						</div>
					) : (
						<p className="mt-4 text-sm text-th-text-muted">
							No active notifications.
						</p>
					)}
				</>
			)}
		</section>
	);
}

export default NotificationSummary;
