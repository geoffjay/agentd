/**
 * TriggerNode — custom React Flow node for workflow trigger sources.
 *
 * Renders a category-coloured card with:
 * - A lucide-react icon matching the trigger type
 * - A human-readable label
 * - A one-line configuration summary
 * - An output handle (right side) for connecting to agent nodes
 * - Selected / hover / disabled visual states
 *
 * All 14 backend trigger types are handled.
 */

import {
	Activity,
	CheckCircle,
	Clock,
	GitFork,
	GitMerge,
	GitPullRequest,
	Hand,
	ListOrdered,
	MessageCircle,
	Moon,
	SquareKanban,
	Timer,
	Webhook,
	type LucideIcon,
} from "lucide-react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type {
	TriggerCategory,
	TriggerConfig,
	TriggerType,
} from "@/types/orchestrator";
import { getTriggerLabel } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Node data interface
// ---------------------------------------------------------------------------

export interface TriggerNodeData extends Record<string, unknown> {
	triggerConfig: TriggerConfig;
	/** Display label (defaults to getTriggerLabel if not provided) */
	label?: string;
	category: TriggerCategory;
	enabled: boolean;
	onConfigChange?: (config: TriggerConfig) => void;
}

// ---------------------------------------------------------------------------
// Icon and colour mappings
// ---------------------------------------------------------------------------

const TRIGGER_ICONS: Record<TriggerType, LucideIcon> = {
	github_issues: GitPullRequest,
	github_pull_requests: GitMerge,
	linear_issues: SquareKanban,
	webhook: Webhook,
	cron: Clock,
	delay: Timer,
	agent_lifecycle: Activity,
	agent_idle: Moon,
	dispatch_result: CheckCircle,
	ask_response: MessageCircle,
	manual: Hand,
	queue: ListOrdered,
	composite: GitFork,
};

/** Tailwind utility classes (bg, border, icon colour) per category */
const CATEGORY_COLOURS: Record<
	TriggerCategory,
	{ bg: string; border: string; icon: string; badge: string }
> = {
	external: {
		bg: "bg-blue-50 dark:bg-blue-950",
		border: "border-blue-300 dark:border-blue-700",
		icon: "text-blue-600 dark:text-blue-400",
		badge: "bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
	},
	schedule: {
		bg: "bg-violet-50 dark:bg-violet-950",
		border: "border-violet-300 dark:border-violet-700",
		icon: "text-violet-600 dark:text-violet-400",
		badge: "bg-violet-100 dark:bg-violet-900 text-violet-700 dark:text-violet-300",
	},
	event: {
		bg: "bg-amber-50 dark:bg-amber-950",
		border: "border-amber-300 dark:border-amber-700",
		icon: "text-amber-600 dark:text-amber-400",
		badge: "bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300",
	},
	internal: {
		bg: "bg-slate-50 dark:bg-slate-900",
		border: "border-slate-300 dark:border-slate-700",
		icon: "text-slate-600 dark:text-slate-400",
		badge: "bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300",
	},
};

// ---------------------------------------------------------------------------
// Config summary helpers
// ---------------------------------------------------------------------------

function configSummary(config: TriggerConfig): string {
	switch (config.type) {
		case "github_issues":
		case "github_pull_requests": {
			const parts = [`${config.owner}/${config.repo}`];
			if (config.labels.length > 0) parts.push(config.labels[0]);
			if (config.labels.length > 1) parts.push(`+${config.labels.length - 1}`);
			return parts.join(" ");
		}
		case "cron":
			return config.expression || "No expression";
		case "delay":
			return config.run_at
				? new Date(config.run_at).toLocaleString()
				: "No date set";
		case "webhook":
			return `Source: ${config.source}`;
		case "manual":
			return "Manual trigger";
		case "linear_issues": {
			const parts: string[] = [];
			if (config.team_key) parts.push(config.team_key);
			if (config.project) parts.push(config.project);
			return parts.length > 0 ? parts.join(" / ") : "Linear Issues";
		}
		case "agent_lifecycle":
			return config.event;
		case "agent_idle":
			return `Idle: ${config.idle_seconds}s`;
		case "dispatch_result":
			return config.source_workflow_id
				? `From: ${config.source_workflow_id.slice(0, 8)}…`
				: "Any workflow";
		case "composite":
			return `${config.mode.toUpperCase()} · ${config.triggers.length} trigger${config.triggers.length !== 1 ? "s" : ""}`;
		case "queue":
			return config.queue_name || "No queue";
		case "ask_response":
			return config.category ?? "Any category";
		default:
			return "";
	}
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TriggerNode({
	data,
	selected,
}: NodeProps<TriggerNodeData>) {
	const { triggerConfig, label, category, enabled } = data;
	const colours = CATEGORY_COLOURS[category] ?? CATEGORY_COLOURS.internal;
	const Icon = TRIGGER_ICONS[triggerConfig.type] ?? Hand;
	const displayLabel = label ?? getTriggerLabel(triggerConfig.type);
	const summary = configSummary(triggerConfig);

	return (
		<div
			data-testid="trigger-node"
			data-trigger-type={triggerConfig.type}
			className={[
				"relative min-w-[180px] max-w-[240px] rounded-lg border-2 px-3 py-2.5 shadow-sm transition-all",
				colours.bg,
				selected
					? "border-blue-500 ring-2 ring-blue-300 dark:ring-blue-700"
					: colours.border,
				!enabled ? "opacity-50 grayscale" : "",
			]
				.filter(Boolean)
				.join(" ")}
		>
			{/* Category badge */}
			<span
				className={[
					"absolute -top-2 left-2 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
					colours.badge,
				].join(" ")}
			>
				{category}
			</span>

			{/* Icon + label */}
			<div className="flex items-center gap-2">
				<Icon
					size={16}
					className={colours.icon}
					aria-hidden="true"
				/>
				<span className="text-xs font-semibold text-th-text leading-tight truncate">
					{displayLabel}
				</span>
			</div>

			{/* Config summary */}
			{summary && (
				<p className="mt-1 text-[11px] text-th-text-muted leading-tight truncate">
					{summary}
				</p>
			)}

			{/* Disabled indicator */}
			{!enabled && (
				<p className="mt-1 text-[10px] font-medium text-th-status-warning-text">
					Disabled
				</p>
			)}

			{/* Output handle — right side */}
			<Handle
				type="source"
				position={Position.Right}
				id="out"
				className="!w-3 !h-3 !bg-th-accent !border-2 !border-white"
				data-testid="trigger-node-handle-out"
			/>
		</div>
	);
}

export default TriggerNode;
