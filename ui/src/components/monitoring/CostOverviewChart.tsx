/**
 * CostOverviewChart — Nivo bar chart showing cumulative cost per agent.
 *
 * Y-axis formatted as USD currency.
 * Custom tooltip with cost breakdown.
 */

import { ResponsiveBar } from "@nivo/bar";
import { ChartSkeleton } from "@/components/common/LoadingSkeleton";
import { useNivoTheme } from "@/hooks/useNivoTheme";
import type { AgentUsageEntry, AggregateUsage } from "@/hooks/useUsageMetrics";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CostOverviewChartProps {
	entries: AgentUsageEntry[];
	aggregate: AggregateUsage;
	loading?: boolean;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BAR_COLOR = "#3b82f6"; // blue-500

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function truncateLabel(name: string, maxLen = 12): string {
	return name.length > maxLen ? `${name.slice(0, maxLen)}…` : name;
}

function formatUsd(value: number): string {
	if (value < 0.01 && value > 0) return `$${value.toFixed(4)}`;
	return `$${value.toFixed(2)}`;
}

function formatTokenCount(value: number): string {
	if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
	if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
	return String(value);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CostOverviewChart({
	entries,
	aggregate,
	loading,
}: CostOverviewChartProps) {
	const nivoTheme = useNivoTheme();

	const barData = entries
		.filter((entry) => entry.stats.cumulative.total_cost_usd > 0)
		.map((entry) => ({
			agent: truncateLabel(entry.name || entry.agentId),
			agentFull: entry.name || entry.agentId,
			cost: entry.stats.cumulative.total_cost_usd,
			input_tokens: entry.stats.cumulative.input_tokens,
			output_tokens: entry.stats.cumulative.output_tokens,
			sessions: entry.stats.session_count,
		}))
		.sort((a, b) => b.cost - a.cost);

	const hasData = barData.length > 0;

	return (
		<div
			className="rounded-lg border border-th-border bg-th-surface p-5"
			aria-label="Cost overview bar chart"
		>
			<div className="flex items-center justify-between mb-4">
				<h3 className="text-sm font-semibold text-th-text">Cost per Agent</h3>
				{hasData && (
					<span className="text-xs text-th-text-muted">
						Total:{" "}
						<span className="font-medium text-th-text">
							{formatUsd(aggregate.totalCostUsd)}
						</span>
					</span>
				)}
			</div>

			{loading ? (
				<ChartSkeleton height={240} />
			) : !hasData ? (
				<div className="flex items-center justify-center h-60 text-sm text-th-text-faint">
					No cost data available
				</div>
			) : (
				<>
					<div
						style={{ height: 240 }}
						role="img"
						aria-label="Bar chart of cost per agent in USD"
					>
						<ResponsiveBar
							data={barData}
							keys={["cost"]}
							indexBy="agent"
							theme={nivoTheme}
							colors={[BAR_COLOR]}
							margin={{ top: 8, right: 8, bottom: 40, left: 56 }}
							padding={0.3}
							borderRadius={3}
							enableLabel={false}
							axisLeft={{
								tickSize: 0,
								tickPadding: 4,
								tickValues: 5,
								format: (v) => formatUsd(Number(v)),
							}}
							axisBottom={{
								tickSize: 0,
								tickPadding: 6,
								tickRotation: entries.length > 6 ? -35 : 0,
							}}
							tooltip={({ data: rowData }) => {
								const row = rowData as Record<string, unknown>;
								return (
									<div className="rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-xs shadow-lg">
										<div className="font-medium text-th-text mb-1.5">
											{row.agentFull as string}
										</div>
										<div className="space-y-0.5">
											<div className="flex justify-between gap-4">
												<span className="text-th-text-muted">Cost:</span>
												<span className="font-medium text-th-text">
													{formatUsd(row.cost as number)}
												</span>
											</div>
											<div className="flex justify-between gap-4">
												<span className="text-th-text-muted">
													Input tokens:
												</span>
												<span className="text-th-text-secondary">
													{formatTokenCount(row.input_tokens as number)}
												</span>
											</div>
											<div className="flex justify-between gap-4">
												<span className="text-th-text-muted">
													Output tokens:
												</span>
												<span className="text-th-text-secondary">
													{formatTokenCount(row.output_tokens as number)}
												</span>
											</div>
											<div className="flex justify-between gap-4">
												<span className="text-th-text-muted">Sessions:</span>
												<span className="text-th-text-secondary">
													{row.sessions as number}
												</span>
											</div>
										</div>
									</div>
								);
							}}
							role="img"
							ariaLabel="Cost per agent bar chart"
						/>
					</div>

					{/* Summary footer */}
					<div className="mt-3 flex flex-wrap gap-4 text-xs text-th-text-muted">
						<span>
							Agents:{" "}
							<span className="font-medium text-th-text-secondary">
								{barData.length}
							</span>
						</span>
						<span>
							Total tokens:{" "}
							<span className="font-medium text-th-text-secondary">
								{formatTokenCount(aggregate.totalTokens)}
							</span>
						</span>
					</div>
				</>
			)}
		</div>
	);
}

export default CostOverviewChart;
