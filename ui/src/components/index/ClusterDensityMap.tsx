/**
 * ClusterDensityMap — scatter plot of 2D-projected chunk embeddings for a
 * repository.
 *
 * Shows where indexed code chunks cluster in embedding space to help diagnose
 * RAG quality issues.
 *
 * Layout:
 * - Header with repo name, total / sampled stats, and cluster health badge
 * - Colour-by toggle: language | chunk_type
 * - Scatter plot (Nivo ResponsiveScatterPlotCanvas) rendered to an HTML5 canvas
 *   for smooth performance up to ~5 000 points
 * - Legend for the active colour dimension
 * - Hint text
 *
 * The component is self-contained: it calls fetchEmbeddingSample on mount /
 * when repoId changes and cleans up on unmount.
 */

import { ResponsiveScatterPlotCanvas } from "@nivo/scatterplot";
import type {
	ScatterPlotDatum,
	ScatterPlotTooltipProps,
} from "@nivo/scatterplot";
import {
	Activity,
	AlertTriangle,
	CheckCircle,
	ChevronDown,
	ChevronRight,
	Loader2,
	Map as MapIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNivoTheme } from "@/hooks/useNivoTheme";
import type { EmbeddingSamplePoint } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ClusterDensityMapProps {
	repoId: string;
	repoName: string;
	embeddingPoints: EmbeddingSamplePoint[];
	embeddingTotal: number;
	embeddingSampled: number;
	embeddingLoading: boolean;
	embeddingError?: string;
	onFetch: (repoId: string, limit?: number) => Promise<void>;
	onClear: () => void;
}

type ColorBy = "language" | "chunk_type";

// ---------------------------------------------------------------------------
// Chart datum
// ---------------------------------------------------------------------------

interface ChartDatum extends ScatterPlotDatum {
	x: number;
	y: number;
	file_path: string;
	language: string;
	chunk_type: string;
	symbol_name?: string;
	colorKey: string; // resolved colour-by value
}

// ---------------------------------------------------------------------------
// Colour palettes
// ---------------------------------------------------------------------------

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

const CHUNK_TYPE_COLORS: Record<string, string> = {
	function: "#3b82f6",
	method: "#6366f1",
	class: "#f97316",
	struct: "#10b981",
	enum: "#eab308",
	trait: "#ec4899",
	impl: "#8b5cf6",
	module: "#06b6d4",
	file: "#64748b",
};

const DEFAULT_COLOR = "#94a3b8";

function colorFor(key: string, colorBy: ColorBy): string {
	const map = colorBy === "language" ? LANGUAGE_COLORS : CHUNK_TYPE_COLORS;
	return map[key.toLowerCase()] ?? DEFAULT_COLOR;
}

// ---------------------------------------------------------------------------
// Cluster health indicator
// ---------------------------------------------------------------------------

type HealthLevel = "good" | "moderate" | "poor";

interface ClusterHealth {
	level: HealthLevel;
	label: string;
	spread: number;
}

function analyseClusterHealth(points: EmbeddingSamplePoint[]): ClusterHealth {
	if (points.length < 10) {
		return { level: "good", label: "Insufficient data", spread: 1 };
	}

	// Compute mean inter-point distance (sampled pairwise for performance).
	const sample = points.slice(0, Math.min(200, points.length));
	let totalDist = 0;
	let pairs = 0;
	for (let i = 0; i < sample.length; i++) {
		for (let j = i + 1; j < Math.min(i + 10, sample.length); j++) {
			const dx = sample[i].x - sample[j].x;
			const dy = sample[i].y - sample[j].y;
			totalDist += Math.sqrt(dx * dx + dy * dy);
			pairs++;
		}
	}
	const avgDist = pairs > 0 ? totalDist / pairs : 1;
	// In a uniform [-1,1]² space the expected average distance is ~0.9.
	const spread = Math.min(avgDist / 0.9, 1);

	if (spread >= 0.6) return { level: "good", label: "Good spread", spread };
	if (spread >= 0.3) return { level: "moderate", label: "Moderate clustering", spread };
	return { level: "poor", label: "High clustering — may affect search quality", spread };
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

function DensityTooltip({ node }: ScatterPlotTooltipProps<ChartDatum>) {
	const { file_path, language, chunk_type, symbol_name } = node.data;
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

function buildSeries(
	points: EmbeddingSamplePoint[],
	colorBy: ColorBy,
): ChartSeries[] {
	const byKey = new Map<string, ChartSeries>();

	for (const p of points) {
		const key = colorBy === "language" ? p.language || "unknown" : p.chunk_type || "unknown";
		if (!byKey.has(key)) {
			byKey.set(key, {
				id: key,
				color: colorFor(key, colorBy),
				data: [],
			});
		}
		byKey.get(key)!.data.push({
			x: p.x,
			y: p.y,
			file_path: p.file_path,
			language: p.language,
			chunk_type: p.chunk_type,
			symbol_name: p.symbol_name,
			colorKey: key,
		});
	}

	return Array.from(byKey.values());
}

// ---------------------------------------------------------------------------
// Health badge
// ---------------------------------------------------------------------------

function HealthBadge({ health }: { health: ClusterHealth }) {
	const cfg = {
		good: {
			icon: <CheckCircle size={12} />,
			className: "text-th-status-success-text bg-th-status-success-bg border-th-status-success-border",
		},
		moderate: {
			icon: <Activity size={12} />,
			className: "text-th-status-warning-text bg-th-status-warning-bg border-th-status-warning-border",
		},
		poor: {
			icon: <AlertTriangle size={12} />,
			className: "text-th-status-error-text bg-th-status-error-bg border-th-status-error-border",
		},
	}[health.level];

	return (
		<span
			className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium ${cfg.className}`}
		>
			{cfg.icon}
			{health.label}
		</span>
	);
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

function DensityLegend({ series }: { series: ChartSeries[] }) {
	return (
		<div className="flex flex-wrap gap-3">
			{series.map((s) => (
				<span
					key={s.id}
					className="flex items-center gap-1.5 text-xs text-th-text-muted"
				>
					<span
						className="h-2.5 w-2.5 rounded-full flex-shrink-0 opacity-70"
						style={{ background: s.color }}
					/>
					{s.id}
					<span className="opacity-50">({s.data.length})</span>
				</span>
			))}
		</div>
	);
}

// ---------------------------------------------------------------------------
// ClusterDensityMap
// ---------------------------------------------------------------------------

export function ClusterDensityMap({
	repoId,
	repoName,
	embeddingPoints,
	embeddingTotal,
	embeddingSampled,
	embeddingLoading,
	embeddingError,
	onFetch,
	onClear,
}: ClusterDensityMapProps) {
	const [visible, setVisible] = useState(true);
	const [colorBy, setColorBy] = useState<ColorBy>("language");
	const nivoTheme = useNivoTheme();

	// Fetch on mount / repo change
	useEffect(() => {
		void onFetch(repoId);
		return () => {
			onClear();
		};
	}, [repoId, onFetch, onClear]);

	const series = useMemo(
		() => buildSeries(embeddingPoints, colorBy),
		[embeddingPoints, colorBy],
	);

	const health = useMemo(
		() => analyseClusterHealth(embeddingPoints),
		[embeddingPoints],
	);

	const refetch = useCallback(() => {
		void onFetch(repoId);
	}, [repoId, onFetch]);

	return (
		<div className="rounded-lg border border-th-border bg-th-surface overflow-hidden">
			{/* Header */}
			<div className="flex items-center gap-3 px-5 py-3 border-b border-th-border flex-wrap">
				<button
					type="button"
					onClick={() => setVisible((v) => !v)}
					className="flex items-center gap-2 text-sm font-medium text-th-text-secondary hover:text-th-text transition-colors"
					aria-expanded={visible}
					aria-controls="density-map-body"
				>
					<MapIcon size={14} aria-hidden="true" />
					Embedding Distribution
					{visible ? (
						<ChevronDown size={14} aria-hidden="true" />
					) : (
						<ChevronRight size={14} aria-hidden="true" />
					)}
				</button>

				{embeddingPoints.length > 0 && !embeddingLoading && (
					<HealthBadge health={health} />
				)}

				<span className="ml-auto text-xs text-th-text-muted">
					{embeddingLoading ? (
						<span className="flex items-center gap-1">
							<Loader2 size={11} className="animate-spin" />
							Loading…
						</span>
					) : embeddingPoints.length > 0 ? (
						`${embeddingSampled.toLocaleString()} / ${embeddingTotal.toLocaleString()} chunks`
					) : null}
				</span>
			</div>

			{visible && (
				<div id="density-map-body" className="px-5 pb-5 pt-4 space-y-4">
					{/* Error */}
					{embeddingError && (
						<div
							role="alert"
							className="rounded-md bg-th-status-error-bg px-3 py-2 text-sm text-th-status-error-text"
						>
							{embeddingError}
							<button
								type="button"
								onClick={refetch}
								className="ml-2 underline underline-offset-2 text-xs hover:opacity-80"
							>
								Retry
							</button>
						</div>
					)}

					{/* Loading skeleton */}
					{embeddingLoading && (
						<div
							className="flex items-center justify-center"
							style={{ height: 300 }}
							aria-label="Loading embedding data"
						>
							<Loader2 size={24} className="animate-spin text-th-text-muted" />
						</div>
					)}

					{/* Chart */}
					{!embeddingLoading && embeddingPoints.length > 0 && (
						<>
							{/* Colour-by toggle */}
							<div
								className="flex items-center gap-2"
								role="group"
								aria-label="Colour by"
							>
								<span className="text-xs text-th-text-muted">Colour by</span>
								<div className="flex items-center rounded-md border border-th-border overflow-hidden text-xs">
									{(["language", "chunk_type"] as ColorBy[]).map((opt) => (
										<button
											key={opt}
											type="button"
											onClick={() => setColorBy(opt)}
											aria-pressed={colorBy === opt}
											className={[
												"px-2.5 py-1 transition-colors",
												colorBy === opt
													? "bg-th-accent/10 text-th-text-link font-medium"
													: "text-th-text-muted hover:text-th-text",
											].join(" ")}
										>
											{opt === "language" ? "Language" : "Chunk type"}
										</button>
									))}
								</div>
							</div>

							{/* Plot */}
							<div
								style={{ height: 320 }}
								aria-label={`Embedding distribution scatter plot for ${repoName}`}
							>
								<ResponsiveScatterPlotCanvas<ChartDatum>
									data={series}
									theme={nivoTheme}
									colors={({ serieId }: { serieId: string | number }) =>
										colorFor(String(serieId), colorBy)
									}
									xScale={{ type: "linear", min: -1.1, max: 1.1 }}
									yScale={{ type: "linear", min: -1.1, max: 1.1 }}
									nodeSize={6}
									margin={{ top: 16, right: 24, bottom: 48, left: 48 }}
									axisBottom={{
										tickSize: 0,
										tickPadding: 6,
										legend: "Projection X",
										legendOffset: 36,
										legendPosition: "middle",
										tickValues: 5,
									}}
									axisLeft={{
										tickSize: 0,
										tickPadding: 6,
										legend: "Projection Y",
										legendOffset: -40,
										legendPosition: "middle",
										tickValues: 5,
									}}
									enableGridX={true}
									tooltip={DensityTooltip}
									role="img"
								/>
							</div>

							<DensityLegend series={series} />

							<p className="text-xs text-th-text-muted opacity-60">
								Overlapping points indicate clusters. High overlap may reduce
								search quality. Coordinates are hash-projected — not raw
								embeddings.
							</p>
						</>
					)}

					{/* Empty state */}
					{!embeddingLoading && !embeddingError && embeddingPoints.length === 0 && (
						<div className="flex flex-col items-center justify-center py-12 text-center">
							<MapIcon
								size={32}
								className="text-th-text-muted opacity-30 mb-3"
								aria-hidden="true"
							/>
							<p className="text-sm text-th-text-muted">No embedding data</p>
							<p className="mt-1 text-xs text-th-text-muted opacity-70">
								Index some code to see the embedding distribution.
							</p>
							<button
								type="button"
								onClick={refetch}
								className="mt-3 text-xs text-th-text-link underline underline-offset-2 hover:opacity-80 transition-opacity"
							>
								Retry
							</button>
						</div>
					)}
				</div>
			)}
		</div>
	);
}

export default ClusterDensityMap;
