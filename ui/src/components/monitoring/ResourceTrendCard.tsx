/**
 * ResourceTrendCard — compact time-series card for one system resource
 * (CPU, memory, load average) fed from the monitor service's history
 * ring buffer.
 *
 * Renders latest-value readouts above a small Nivo line chart. Shows a
 * quiet empty state when the monitor is unreachable or has no snapshots
 * yet — it never crashes the page.
 */

import { ResponsiveLine } from "@nivo/line";
import { Activity } from "lucide-react";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import { useNivoTheme } from "@/hooks/useNivoTheme";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface TrendSeries {
	id: string;
	color: string;
	data: Array<{ x: Date; y: number }>;
}

export interface TrendReadout {
	label: string;
	value: string;
	color: string;
}

export interface ResourceTrendCardProps {
	title: string;
	description?: string;
	series: TrendSeries[];
	/** Latest-value readouts shown above the chart (double as the legend) */
	readouts: TrendReadout[];
	/**
	 * Upper bound for the y axis. Percentage charts pass 100; omit for an
	 * auto-scaled axis (load average).
	 */
	yMax?: number;
	/** Suffix appended to tooltip values (e.g. "%") */
	unit?: string;
	/** False when the monitor service is unreachable */
	available?: boolean;
	loading?: boolean;
	/** Height of the chart area in pixels (default 140) */
	height?: number;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ResourceTrendCard({
	title,
	description,
	series,
	readouts,
	yMax,
	unit = "",
	available = true,
	loading = false,
	height = 140,
}: ResourceTrendCardProps) {
	const nivoTheme = useNivoTheme();
	const hasData = available && series.some((s) => s.data.length > 0);

	return (
		<div className="rounded-lg border border-th-border bg-th-surface p-4">
			{/* Header */}
			<h3 className="text-sm font-semibold text-th-text">{title}</h3>
			{description && (
				<p className="mt-0.5 text-xs text-th-text-faint">{description}</p>
			)}

			{/* Loading */}
			{loading && (
				<div className="mt-3">
					<ChartSkeleton height={height} />
				</div>
			)}

			{/* Empty / unavailable state */}
			{!loading && !hasData && (
				<div
					className="mt-3 flex flex-col items-center justify-center gap-1.5 text-center"
					style={{ height }}
				>
					<Activity size={20} className="text-th-text-faint" />
					<p className="text-xs text-th-text-muted">
						{available
							? "No metrics collected yet."
							: "Monitor service unavailable"}
					</p>
				</div>
			)}

			{/* Content */}
			{!loading && hasData && (
				<>
					<div className="mt-2 flex flex-wrap items-center gap-3">
						{readouts.map((r) => (
							<div
								key={r.label}
								className="flex items-center gap-1.5 text-xs text-th-text-muted"
							>
								<span
									aria-hidden="true"
									className="h-2 w-2 rounded-full"
									style={{ background: r.color }}
								/>
								<span>{r.label}</span>
								<span className="font-semibold text-th-text">{r.value}</span>
							</div>
						))}
					</div>

					<div
						className="mt-2"
						style={{ height }}
						data-testid={`resource-trend-${title.toLowerCase().replace(/\s+/g, "-")}`}
					>
						<ResponsiveLine
							data={series}
							theme={nivoTheme}
							colors={(serie: { color: string }) => serie.color}
							xScale={{ type: "time", format: "native", precision: "minute" }}
							yScale={{ type: "linear", min: 0, max: yMax ?? "auto" }}
							axisBottom={{
								format: "%H:%M",
								tickValues: 3,
								tickSize: 0,
								tickPadding: 6,
							}}
							axisLeft={{
								tickValues: 3,
								tickSize: 0,
								tickPadding: 4,
								format: (v: number) => `${v}${unit}`,
							}}
							enableGridX={false}
							gridYValues={3}
							lineWidth={1.5}
							enablePoints={false}
							enableArea
							areaOpacity={0.1}
							curve="monotoneX"
							useMesh
							margin={{ top: 6, right: 6, bottom: 24, left: 34 }}
							tooltip={({ point }) => (
								<div className="rounded bg-th-surface-raised px-2 py-1 text-xs text-th-text shadow">
									{String(point.seriesId)}: {Number(point.data.y).toFixed(1)}
									{unit}
								</div>
							)}
						/>
					</div>
				</>
			)}
		</div>
	);
}

export default ResourceTrendCard;
