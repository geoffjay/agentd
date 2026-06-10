/**
 * ActivityChart — stacked bar chart of activity counts per hour over the
 * last 24 hours, split by type (notifications, questions, agent updates).
 *
 * Uses the muted theme palette so the chart sits quietly on the page;
 * full-strength colors appear only in the small inline legend dots.
 */

import { ResponsiveBar } from "@nivo/bar";
import { BarChart2 } from "lucide-react";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import type { ActivityBucket } from "@/hooks/useActivityFeed";
import { useChartPalette } from "@/hooks/useChartPalette";
import { useNivoTheme } from "@/hooks/useNivoTheme";

/** Alpha used for bar fills */
const FILL_ALPHA = 0.7;

const KEYS = ["notifications", "questions", "agents"] as const;

const KEY_LABELS: Record<(typeof KEYS)[number], string> = {
	notifications: "Notifications",
	questions: "Questions",
	agents: "Agents",
};

interface ActivityChartProps {
	buckets: ActivityBucket[];
	loading?: boolean;
	error?: string;
}

export function ActivityChart({ buckets, loading, error }: ActivityChartProps) {
	const palette = useChartPalette();
	const nivoTheme = useNivoTheme();

	// Full-strength colors per series (legend); fills are muted below
	const keyColors: Record<(typeof KEYS)[number], string> = {
		notifications: palette.info,
		questions: palette.warning,
		agents: palette.accent,
	};

	const hasData = buckets.some(
		(b) => b.notifications + b.questions + b.agents > 0,
	);

	// Only label every 4th hour to keep the axis quiet
	const tickValues = buckets.flatMap((b, i) => (i % 4 === 0 ? [b.hour] : []));

	return (
		<section
			aria-labelledby="activity-chart-heading"
			className="rounded-lg border border-th-border bg-th-surface p-5"
		>
			{/* Header + inline legend */}
			<div className="flex flex-wrap items-center justify-between gap-2">
				<h2
					id="activity-chart-heading"
					className="text-base font-semibold text-th-text"
				>
					Activity (24h)
				</h2>
				<div className="flex items-center gap-3">
					{KEYS.map((key) => (
						<span
							key={key}
							className="flex items-center gap-1 text-xs text-th-text-muted"
						>
							<span
								aria-hidden="true"
								className="h-2 w-2 rounded-full"
								style={{ background: keyColors[key] }}
							/>
							{KEY_LABELS[key]}
						</span>
					))}
				</div>
			</div>

			{/* Error */}
			{error && (
				<p className="mt-3 text-sm text-th-status-error-text">{error}</p>
			)}

			{/* Loading */}
			{loading && !error && (
				<div className="mt-4">
					<ChartSkeleton height={192} />
				</div>
			)}

			{/* Empty state */}
			{!loading && !error && !hasData && (
				<div className="mt-4 flex h-48 flex-col items-center justify-center gap-2 text-center">
					<BarChart2 size={24} className="text-th-text-faint" />
					<p className="text-sm text-th-text-muted">
						No activity in the last 24 hours.
					</p>
				</div>
			)}

			{/* Chart */}
			{!loading && !error && hasData && (
				<div className="mt-4 h-48" data-testid="activity-chart">
					<ResponsiveBar
						data={buckets as unknown as Record<string, number | string>[]}
						keys={[...KEYS]}
						indexBy="hour"
						groupMode="stacked"
						theme={nivoTheme}
						colors={({ id }) =>
							palette.withAlpha(
								keyColors[id as (typeof KEYS)[number]] ?? palette.neutral,
								FILL_ALPHA,
							)
						}
						enableLabel={false}
						borderRadius={2}
						padding={0.35}
						axisLeft={{
							tickSize: 0,
							tickPadding: 6,
							tickValues: 4,
						}}
						axisBottom={{
							tickSize: 0,
							tickPadding: 8,
							tickValues,
						}}
						margin={{ top: 8, right: 8, bottom: 28, left: 28 }}
						tooltip={({ id, indexValue, value }) => (
							<div className="rounded bg-th-surface-raised px-2 py-1 text-xs text-th-text shadow">
								{KEY_LABELS[id as (typeof KEYS)[number]] ?? id} at {indexValue}:{" "}
								{value}
							</div>
						)}
					/>
				</div>
			)}
		</section>
	);
}

export default ActivityChart;
