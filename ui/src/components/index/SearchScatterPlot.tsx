/**
 * SearchScatterPlot — scatter plot visualisation of code search results.
 *
 * Each result is a point on the chart:
 *   X-axis  : result rank (1, 2, 3 …)
 *   Y-axis  : similarity score (0.0 – 1.0)
 *   Size    : proportional to chunk line count (end_line - start_line)
 *   Colour  : language (Rust=orange, Python=blue, TypeScript=cyan …)
 *
 * The chart is shown below the results table and can be toggled on/off.
 * Returns null when there are no results.
 */

import { ResponsiveScatterPlot } from "@nivo/scatterplot";
import type {
	ScatterPlotDatum,
	ScatterPlotNodeData,
	ScatterPlotTooltipProps,
} from "@nivo/scatterplot";
import { ChartScatter, ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import { useNivoTheme } from "@/hooks/useNivoTheme";
import type { CodeSearchResultItem } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Chart datum type
// ---------------------------------------------------------------------------

interface ChartDatum extends ScatterPlotDatum {
	x: number;
	y: number;
	file_path: string;
	symbol_name?: string;
	score: number;
	chunk_type: string;
	language: string;
	lines: number;
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface SearchScatterPlotProps {
	results: CodeSearchResultItem[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Language -> colour mapping. Chosen to be visible in both light and dark themes. */
const LANGUAGE_COLORS: Record<string, string> = {
	rust: "#f97316",
	python: "#3b82f6",
	typescript: "#06b6d4",
	javascript: "#eab308",
	go: "#10b981",
	ruby: "#ef4444",
	java: "#8b5cf6",
	cpp: "#ec4899",
	c: "#64748b",
	shell: "#84cc16",
	toml: "#a78bfa",
	yaml: "#f59e0b",
	markdown: "#94a3b8",
};

const DEFAULT_COLOR = "#94a3b8";

function langColor(lang: string): string {
	return LANGUAGE_COLORS[lang.toLowerCase()] ?? DEFAULT_COLOR;
}

const NODE_MIN = 6;
const NODE_MAX = 22;

function chunkLines(item: CodeSearchResultItem): number {
	return Math.max(1, item.end_line - item.start_line + 1);
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

function ScatterTooltip({ node }: ScatterPlotTooltipProps<ChartDatum>) {
	const { file_path, symbol_name, score, chunk_type, language } = node.data;
	return (
		<div className="rounded-md border border-th-border bg-th-surface-raised px-3 py-2 text-xs shadow-md space-y-0.5 max-w-xs">
			<p className="font-mono font-medium text-th-text truncate">{file_path}</p>
			{symbol_name && (
				<p className="font-mono text-th-text-secondary">{symbol_name}</p>
			)}
			<div className="flex items-center gap-2 pt-0.5">
				<span
					className="h-2 w-2 rounded-full flex-shrink-0"
					style={{ background: node.color }}
				/>
				<span className="text-th-text-muted">{language}</span>
				<span className="text-th-text-muted">·</span>
				<span className="text-th-text-muted">{chunk_type}</span>
				<span className="text-th-text-muted">·</span>
				<span className="font-medium text-th-text-link">
					{(score * 100).toFixed(1)}%
				</span>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Series builder
// ---------------------------------------------------------------------------

interface ChartSeries {
	id: string;
	color: string;
	data: ChartDatum[];
}

function buildSeries(results: CodeSearchResultItem[]): ChartSeries[] {
	const byLang = new Map<string, ChartSeries>();

	results.forEach((item, idx) => {
		const lang = item.language || "unknown";
		if (!byLang.has(lang)) {
			byLang.set(lang, { id: lang, color: langColor(lang), data: [] });
		}
		byLang.get(lang)!.data.push({
			x: idx + 1,
			y: item.score,
			file_path: item.file_path,
			symbol_name: item.symbol_name,
			score: item.score,
			chunk_type: item.chunk_type,
			language: lang,
			lines: chunkLines(item),
		});
	});

	return Array.from(byLang.values());
}

/** Returns a nodeSize accessor correctly typed for ChartDatum. */
function makeNodeSizeFn(results: CodeSearchResultItem[]) {
	const sizes = results.map(chunkLines);
	const minS = Math.min(...sizes);
	const maxS = Math.max(...sizes);
	const range = maxS - minS || 1;

	return (
		node: Omit<ScatterPlotNodeData<ChartDatum>, "size" | "color">,
	): number => {
		const lines = node.data.lines;
		const t = (lines - minS) / range;
		return Math.round(NODE_MIN + t * (NODE_MAX - NODE_MIN));
	};
}

// ---------------------------------------------------------------------------
// Legend row
// ---------------------------------------------------------------------------

function ScatterLegend({ series }: { series: ChartSeries[] }) {
	return (
		<div className="flex flex-wrap gap-3">
			{series.map((s) => (
				<span
					key={s.id}
					className="flex items-center gap-1.5 text-xs text-th-text-muted"
				>
					<span
						className="h-2.5 w-2.5 rounded-full flex-shrink-0"
						style={{ background: s.color }}
					/>
					{s.id}
				</span>
			))}
		</div>
	);
}

// ---------------------------------------------------------------------------
// SearchScatterPlot
// ---------------------------------------------------------------------------

export function SearchScatterPlot({ results }: SearchScatterPlotProps) {
	const [visible, setVisible] = useState(true);
	const nivoTheme = useNivoTheme();

	const series = useMemo(() => buildSeries(results), [results]);
	const nodeSizeFn = useMemo(() => makeNodeSizeFn(results), [results]);

	if (results.length === 0) return null;

	return (
		<div className="rounded-lg border border-th-border bg-th-surface overflow-hidden">
			{/* Header / toggle */}
			<button
				type="button"
				onClick={() => setVisible((v) => !v)}
				className="flex w-full items-center gap-2 px-5 py-3 text-sm font-medium text-th-text-secondary hover:bg-th-surface-hover transition-colors"
				aria-expanded={visible}
				aria-controls="scatter-plot-body"
			>
				<ChartScatter size={14} aria-hidden="true" />
				Score Distribution
				<span className="ml-auto text-th-text-muted">
					{visible ? (
						<ChevronDown size={14} aria-hidden="true" />
					) : (
						<ChevronRight size={14} aria-hidden="true" />
					)}
				</span>
			</button>

			{visible && (
				<div id="scatter-plot-body" className="px-5 pb-5 space-y-3">
					<div style={{ height: 260 }} aria-label="Search results scatter plot">
						<ResponsiveScatterPlot<ChartDatum>
							data={series}
							theme={nivoTheme}
							colors={({ serieId }: { serieId: string | number }) =>
								langColor(String(serieId))
							}
							xScale={{ type: "linear", min: 0, max: results.length + 1 }}
							yScale={{ type: "linear", min: 0, max: 1 }}
							nodeSize={nodeSizeFn}
							margin={{ top: 16, right: 24, bottom: 48, left: 52 }}
							axisBottom={{
								tickSize: 0,
								tickPadding: 6,
								legend: "Result rank",
								legendOffset: 36,
								legendPosition: "middle",
								tickValues: Math.min(results.length, 10),
							}}
							axisLeft={{
								tickSize: 0,
								tickPadding: 6,
								legend: "Similarity score",
								legendOffset: -44,
								legendPosition: "middle",
								tickValues: 5,
								format: (v: number | string) =>
									`${(Number(v) * 100).toFixed(0)}%`,
							}}
							enableGridX={false}
							useMesh={false}
							tooltip={ScatterTooltip}
							role="img"
							ariaLabel="Scatter plot of search result scores by rank"
						/>
					</div>

					<ScatterLegend series={series} />

					<p className="text-xs text-th-text-muted opacity-60">
						Point size reflects chunk line count. Hover a point for details.
					</p>
				</div>
			)}
		</div>
	);
}

export default SearchScatterPlot;
