/**
 * SystemMetricsChart — CPU and memory usage over time from the monitor
 * service, rendered as a muted two-series line chart with latest-value
 * readouts above it.
 *
 * Renders a quiet empty state when the monitor service is unreachable or
 * has not collected any metrics yet — it never crashes the dashboard.
 */

import { ResponsiveLine } from "@nivo/line";
import { Activity, Cpu, ExternalLink, MemoryStick } from "lucide-react";
import { Link } from "react-router-dom";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import { useChartPalette } from "@/hooks/useChartPalette";
import { useNivoTheme } from "@/hooks/useNivoTheme";
import type { UseSystemMetricsResult } from "@/hooks/useSystemMetrics";

/** Alpha used for area fills under the lines */
const AREA_ALPHA = 0.12;

type SystemMetricsChartProps = UseSystemMetricsResult;

interface StatReadoutProps {
	icon: React.ReactNode;
	label: string;
	value: string;
	color: string;
}

function StatReadout({ icon, label, value, color }: StatReadoutProps) {
	return (
		<div className="flex items-center gap-1.5 text-xs text-th-text-muted">
			<span
				aria-hidden="true"
				className="h-2 w-2 rounded-full"
				style={{ background: color }}
			/>
			{icon}
			<span>{label}</span>
			<span className="font-semibold text-th-text">{value}</span>
		</div>
	);
}

export function SystemMetricsChart({
	history,
	latest,
	alerts,
	available,
	loading,
}: SystemMetricsChartProps) {
	const palette = useChartPalette();
	const nivoTheme = useNivoTheme();

	const cpuColor = palette.series[0];
	const memColor = palette.series[1];

	const lineData = [
		{
			id: "CPU",
			color: cpuColor,
			data: history.map((m) => ({
				x: new Date(m.collected_at),
				y: m.cpu.usage_percent,
			})),
		},
		{
			id: "Memory",
			color: memColor,
			data: history.map((m) => ({
				x: new Date(m.collected_at),
				y: m.memory.usage_percent,
			})),
		},
	];

	const hasData = available && history.length > 0;

	return (
		<section
			aria-labelledby="system-metrics-heading"
			className="rounded-lg border border-th-border bg-th-surface p-5"
		>
			{/* Header */}
			<div className="flex items-center justify-between">
				<h2
					id="system-metrics-heading"
					className="text-base font-semibold text-th-text"
				>
					System Metrics
				</h2>
				<Link
					to="/monitoring"
					className="flex items-center gap-1 text-xs font-medium text-th-text-link hover:opacity-80"
				>
					Monitoring <ExternalLink size={12} />
				</Link>
			</div>

			{/* Loading */}
			{loading && (
				<div className="mt-4">
					<ChartSkeleton height={192} />
				</div>
			)}

			{/* Empty / unavailable state */}
			{!loading && !hasData && (
				<div className="mt-4 flex h-48 flex-col items-center justify-center gap-2 text-center">
					<Activity size={24} className="text-th-text-faint" />
					<p className="text-sm text-th-text-muted">
						{available
							? "No metrics collected yet."
							: "Monitor service unavailable"}
					</p>
				</div>
			)}

			{/* Content */}
			{!loading && hasData && (
				<>
					{/* Latest readouts (double as the chart legend) */}
					<div className="mt-3 flex flex-wrap items-center gap-4">
						<StatReadout
							icon={<Cpu size={12} />}
							label="CPU"
							value={latest ? `${latest.cpu.usage_percent.toFixed(0)}%` : "—"}
							color={cpuColor}
						/>
						<StatReadout
							icon={<MemoryStick size={12} />}
							label="Memory"
							value={
								latest ? `${latest.memory.usage_percent.toFixed(0)}%` : "—"
							}
							color={memColor}
						/>
						{latest && (
							<span className="text-xs text-th-text-faint">
								load {latest.load_average.one.toFixed(2)}
							</span>
						)}
						{alerts.length > 0 && (
							<span className="rounded-full bg-th-status-warning-bg px-2 py-0.5 text-xs font-medium text-th-status-warning-text">
								{alerts.length} alert{alerts.length === 1 ? "" : "s"}
							</span>
						)}
					</div>

					{/* Time-series chart */}
					<div className="mt-3 h-48" data-testid="system-metrics-chart">
						<ResponsiveLine
							data={lineData}
							theme={nivoTheme}
							colors={(serie: { color: string }) => serie.color}
							xScale={{ type: "time", format: "native", precision: "minute" }}
							yScale={{ type: "linear", min: 0, max: 100 }}
							axisBottom={{
								format: "%H:%M",
								tickValues: 4,
								tickSize: 0,
								tickPadding: 8,
							}}
							axisLeft={{
								tickValues: [0, 50, 100],
								tickSize: 0,
								tickPadding: 6,
								format: (v: number) => `${v}%`,
							}}
							gridYValues={[0, 25, 50, 75, 100]}
							enableGridX={false}
							lineWidth={2}
							enablePoints={false}
							enableArea
							areaOpacity={AREA_ALPHA}
							curve="monotoneX"
							useMesh
							margin={{ top: 8, right: 8, bottom: 28, left: 36 }}
							tooltip={({ point }) => (
								<div className="rounded bg-th-surface-raised px-2 py-1 text-xs text-th-text shadow">
									{String(point.seriesId)}: {Number(point.data.y).toFixed(1)}%
								</div>
							)}
						/>
					</div>
				</>
			)}
		</section>
	);
}

export default SystemMetricsChart;
