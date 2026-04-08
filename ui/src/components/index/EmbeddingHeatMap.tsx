/**
 * EmbeddingHeatMap — hex-bin density heatmap of 2D-projected chunk embeddings.
 *
 * Used for repositories with > 5 000 chunks where rendering individual scatter
 * points would be too slow.  The backend aggregates all chunk IDs into a hex-bin
 * grid and returns only the non-empty cell counts, keeping the payload small even
 * for 100 k-chunk repositories.
 *
 * Rendering is done on an HTML5 canvas using flat-top hexagonal cells.
 * Density is encoded on a blue-to-red colour scale with a legend.
 */

import { ChevronDown, ChevronRight, Loader2, Map as MapIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { EmbeddingHexBinCell } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface EmbeddingHeatMapProps {
	repoId: string;
	repoName: string;
	hexbinCells: EmbeddingHexBinCell[];
	hexbinTotal: number;
	hexbinBinsParam: number;
	hexbinLoading: boolean;
	hexbinError?: string;
	onFetch: (repoId: string, bins?: number) => Promise<void>;
	onClear: () => void;
}

// ---------------------------------------------------------------------------
// Colour scale helpers
// ---------------------------------------------------------------------------

/** Map density t ∈ [0, 1] to an rgba() string on a blue→cyan→green→yellow→red scale. */
function densityColor(t: number): string {
	// Clamp
	const v = Math.max(0, Math.min(1, t));

	// 5-stop colour scale: blue → cyan → green → yellow → red
	const stops: [number, number, number][] = [
		[59, 130, 246],   // blue-500
		[6, 182, 212],    // cyan-500
		[16, 185, 129],   // emerald-500
		[234, 179, 8],    // yellow-500
		[239, 68, 68],    // red-500
	];

	const segment = v * (stops.length - 1);
	const lo = Math.floor(segment);
	const hi = Math.min(lo + 1, stops.length - 1);
	const frac = segment - lo;

	const [r0, g0, b0] = stops[lo];
	const [r1, g1, b1] = stops[hi];

	const r = Math.round(r0 + (r1 - r0) * frac);
	const g = Math.round(g0 + (g1 - g0) * frac);
	const b = Math.round(b0 + (b1 - b0) * frac);

	return `rgba(${r},${g},${b},0.85)`;
}

// ---------------------------------------------------------------------------
// Hex geometry helpers (pointy-top axial coordinates)
// ---------------------------------------------------------------------------

/**
 * Convert axial (q, r) to pixel (cx, cy) for a pointy-top hex of size `s`.
 * The space maps axial coordinates to the canvas; the caller applies an offset.
 */
function axialToPixel(q: number, r: number, s: number): [number, number] {
	const cx = s * (Math.sqrt(3) * q + (Math.sqrt(3) / 2) * r);
	const cy = s * (3 / 2) * r;
	return [cx, cy];
}

/** Draw a single flat-top hex centred at (cx, cy) with outer radius `s`. */
function drawHex(
	ctx: CanvasRenderingContext2D,
	cx: number,
	cy: number,
	s: number,
	fill: string,
): void {
	ctx.beginPath();
	for (let i = 0; i < 6; i++) {
		const angle = (Math.PI / 3) * i - Math.PI / 6; // pointy-top
		const x = cx + s * Math.cos(angle);
		const y = cy + s * Math.sin(angle);
		if (i === 0) ctx.moveTo(x, y);
		else ctx.lineTo(x, y);
	}
	ctx.closePath();
	ctx.fillStyle = fill;
	ctx.fill();
}

// ---------------------------------------------------------------------------
// Canvas renderer
// ---------------------------------------------------------------------------

function renderHeatMap(
	canvas: HTMLCanvasElement,
	cells: EmbeddingHexBinCell[],
	binsParam: number,
): void {
	const ctx = canvas.getContext("2d");
	if (!ctx) return;

	const W = canvas.width;
	const H = canvas.height;

	ctx.clearRect(0, 0, W, H);

	if (cells.length === 0) return;

	const maxCount = Math.max(...cells.map((c) => c.count));
	if (maxCount === 0) return;

	// The axial space spans roughly ±binsParam/2 in each direction.
	// Compute pixel coordinates for each cell and find the bounding box.
	// Hex size in axial space maps to pixels via a scale factor.
	// We want the full range to fit within the canvas with some padding.

	const PADDING = 32; // px
	const SQRT3 = Math.sqrt(3);

	// Hex size in axial coordinates is 2.0 / (sqrt(3) * binsParam) — same as backend.
	// In pixel space we scale so the bounding box fills the canvas.
	// First, figure out axial bounding box.
	let minQ = Infinity, maxQ = -Infinity;
	let minR = Infinity, maxR = -Infinity;
	for (const c of cells) {
		minQ = Math.min(minQ, c.q);
		maxQ = Math.max(maxQ, c.q);
		minR = Math.min(minR, c.r);
		maxR = Math.max(maxR, c.r);
	}

	// Pixel extent of the axial grid at hex size = 1 (unit hexes).
	const pxSpanQ = SQRT3 * (maxQ - minQ + 1);
	const pxSpanR = 1.5 * (maxR - minR + 1) + 0.5;

	const scale = Math.min(
		(W - 2 * PADDING) / (pxSpanQ || 1),
		(H - 2 * PADDING) / (pxSpanR || 1),
	);

	// Compute centre offset so cells are centred in the canvas.
	const midQ = (minQ + maxQ) / 2;
	const midR = (minR + maxR) / 2;
	const [midPx, midPy] = axialToPixel(midQ, midR, scale);
	const offsetX = W / 2 - midPx;
	const offsetY = H / 2 - midPy;

	// Hex outer radius in pixels (slightly shrunk for gaps).
	const hexR = scale * 0.94;

	for (const cell of cells) {
		const t = Math.log1p(cell.count) / Math.log1p(maxCount);
		const color = densityColor(t);
		const [px, py] = axialToPixel(cell.q, cell.r, scale);
		drawHex(ctx, px + offsetX, py + offsetY, hexR, color);
	}
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

function HexBinLegend({ maxCount }: { maxCount: number }) {
	const stops = [0, 0.25, 0.5, 0.75, 1.0];
	return (
		<div className="flex items-center gap-1">
			<span className="text-xs text-th-text-muted mr-1">Low</span>
			{stops.map((t) => (
				<span
					key={t}
					className="h-3 w-5 rounded-sm inline-block"
					style={{ background: densityColor(t) }}
					title={`${Math.round(Math.exp(t * Math.log1p(maxCount)) - 1)} chunks`}
				/>
			))}
			<span className="text-xs text-th-text-muted ml-1">
				High ({maxCount.toLocaleString()})
			</span>
		</div>
	);
}

// ---------------------------------------------------------------------------
// EmbeddingHeatMap
// ---------------------------------------------------------------------------

export function EmbeddingHeatMap({
	repoId,
	repoName,
	hexbinCells,
	hexbinTotal,
	hexbinBinsParam,
	hexbinLoading,
	hexbinError,
	onFetch,
	onClear,
}: EmbeddingHeatMapProps) {
	const [visible, setVisible] = useState(true);
	const canvasRef = useRef<HTMLCanvasElement>(null);

	// Fetch on mount / repo change.
	useEffect(() => {
		void onFetch(repoId);
		return () => {
			onClear();
		};
	}, [repoId, onFetch, onClear]);

	// Re-render whenever cell data changes.
	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas || hexbinCells.length === 0) return;
		renderHeatMap(canvas, hexbinCells, hexbinBinsParam);
	}, [hexbinCells, hexbinBinsParam]);

	const refetch = useCallback(() => {
		void onFetch(repoId);
	}, [repoId, onFetch]);

	const maxCount =
		hexbinCells.length > 0 ? Math.max(...hexbinCells.map((c) => c.count)) : 0;

	return (
		<div className="rounded-lg border border-th-border bg-th-surface overflow-hidden">
			{/* Header */}
			<div className="flex items-center gap-3 px-5 py-3 border-b border-th-border flex-wrap">
				<button
					type="button"
					onClick={() => setVisible((v) => !v)}
					className="flex items-center gap-2 text-sm font-medium text-th-text-secondary hover:text-th-text transition-colors"
					aria-expanded={visible}
					aria-controls="heatmap-body"
				>
					<MapIcon size={14} aria-hidden="true" />
					Density Heatmap
					{visible ? (
						<ChevronDown size={14} aria-hidden="true" />
					) : (
						<ChevronRight size={14} aria-hidden="true" />
					)}
				</button>

				<span className="ml-auto text-xs text-th-text-muted">
					{hexbinLoading ? (
						<span className="flex items-center gap-1">
							<Loader2 size={11} className="animate-spin" />
							Loading…
						</span>
					) : hexbinTotal > 0 ? (
						`${hexbinTotal.toLocaleString()} chunks · ${hexbinCells.length} bins`
					) : null}
				</span>
			</div>

			{visible && (
				<div id="heatmap-body" className="px-5 pb-5 pt-4 space-y-4">
					{/* Error */}
					{hexbinError && (
						<div
							role="alert"
							className="rounded-md bg-th-status-error-bg px-3 py-2 text-sm text-th-status-error-text"
						>
							{hexbinError}
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
					{hexbinLoading && (
						<div
							className="flex items-center justify-center"
							style={{ height: 320 }}
							aria-label="Loading density heatmap"
						>
							<Loader2 size={24} className="animate-spin text-th-text-muted" />
						</div>
					)}

					{/* Canvas */}
					{!hexbinLoading && hexbinCells.length > 0 && (
						<>
							<div
								aria-label={`Density heatmap for ${repoName}`}
								style={{ height: 320 }}
								className="w-full"
							>
								<canvas
									ref={canvasRef}
									width={700}
									height={320}
									className="w-full h-full"
									aria-hidden="true"
								/>
							</div>

							<HexBinLegend maxCount={maxCount} />

							<p className="text-xs text-th-text-muted opacity-60">
								Each cell shows how many chunks project into that region of
								embedding space. High-density clusters may affect search quality.
								Coordinates are hash-projected — not raw embeddings.
							</p>
						</>
					)}

					{/* Empty state */}
					{!hexbinLoading && !hexbinError && hexbinCells.length === 0 && (
						<div className="flex flex-col items-center justify-center py-12 text-center">
							<MapIcon
								size={32}
								className="text-th-text-muted opacity-30 mb-3"
								aria-hidden="true"
							/>
							<p className="text-sm text-th-text-muted">No embedding data</p>
							<p className="mt-1 text-xs text-th-text-muted opacity-70">
								Index some code to see the density heatmap.
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

export default EmbeddingHeatMap;
