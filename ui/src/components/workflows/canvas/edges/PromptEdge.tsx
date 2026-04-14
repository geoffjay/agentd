/**
 * PromptEdge — custom React Flow edge for workflow prompt-template bindings.
 *
 * Visual design:
 * - Bezier curve using React Flow's getBezierPath
 * - Edge label shows a truncated preview of the prompt template
 * - Small poll-interval badge on the path
 * - Click handler to open the prompt template editor
 * - Visually distinct for empty/default vs. customised prompts
 */

import {
	BaseEdge,
	EdgeLabelRenderer,
	getBezierPath,
	type EdgeProps,
} from "@xyflow/react";

// ---------------------------------------------------------------------------
// Edge data interface
// ---------------------------------------------------------------------------

export interface PromptEdgeData extends Record<string, unknown> {
	promptTemplate: string;
	pollIntervalSecs: number;
	enabled: boolean;
	onPromptChange?: (template: string) => void;
	onIntervalChange?: (secs: number) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TRUNCATE_CHARS = 40;

function truncateTemplate(template: string): string {
	const first = template.split("\n")[0].trim();
	if (first.length <= TRUNCATE_CHARS) return first;
	return `${first.slice(0, TRUNCATE_CHARS)}…`;
}

function formatInterval(secs: number): string {
	if (secs < 60) return `${secs}s`;
	const mins = Math.round(secs / 60);
	if (mins < 60) return `${mins}m`;
	return `${Math.round(mins / 60)}h`;
}

/** Returns true when the template is non-empty and not purely the default placeholder */
function isCustomised(template: string): boolean {
	return template.trim().length > 0;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function PromptEdge({
	id,
	sourceX,
	sourceY,
	targetX,
	targetY,
	sourcePosition,
	targetPosition,
	data,
	selected,
	markerEnd,
}: EdgeProps<PromptEdgeData>) {
	const { promptTemplate = "", pollIntervalSecs = 60, enabled = true } =
		data ?? {};

	const [edgePath, labelX, labelY] = getBezierPath({
		sourceX,
		sourceY,
		sourcePosition,
		targetX,
		targetY,
		targetPosition,
	});

	const customised = isCustomised(promptTemplate);
	const preview = customised
		? truncateTemplate(promptTemplate)
		: "No prompt set";

	const strokeColour = selected
		? "var(--th-accent)"
		: !enabled
			? "var(--th-border)"
			: customised
				? "var(--th-accent)"
				: "var(--th-text-muted)";

	const strokeWidth = selected ? 2.5 : 1.5;
	const strokeDasharray = enabled ? undefined : "5 4";

	function handleClick() {
		data?.onPromptChange?.(promptTemplate);
	}

	return (
		<>
			<BaseEdge
				id={id}
				path={edgePath}
				markerEnd={markerEnd}
				style={{
					stroke: strokeColour,
					strokeWidth,
					strokeDasharray,
				}}
			/>

			<EdgeLabelRenderer>
				{/* Template preview label */}
				<div
					data-testid="prompt-edge-label"
					style={{
						position: "absolute",
						transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
						pointerEvents: "all",
					}}
					className="nodrag nopan"
				>
					<button
						type="button"
						onClick={handleClick}
						title={promptTemplate || "Click to edit prompt template"}
						className={[
							"max-w-[160px] rounded px-2 py-0.5 text-[11px] leading-tight truncate",
							"border shadow-sm transition-colors",
							customised
								? "bg-th-surface border-th-accent text-th-text"
								: "bg-th-surface-sunken border-th-border text-th-text-muted",
							"hover:border-th-accent hover:text-th-text",
						].join(" ")}
					>
						{preview}
					</button>
				</div>

				{/* Poll interval badge — offset slightly above mid-point */}
				<div
					data-testid="prompt-edge-interval"
					style={{
						position: "absolute",
						transform: `translate(-50%, -220%) translate(${labelX}px, ${labelY}px)`,
						pointerEvents: "none",
					}}
					className="nodrag nopan"
				>
					<span className="rounded-full bg-th-surface border border-th-border px-1.5 py-0.5 text-[10px] text-th-text-faint">
						{formatInterval(pollIntervalSecs)}
					</span>
				</div>
			</EdgeLabelRenderer>
		</>
	);
}

export default PromptEdge;
