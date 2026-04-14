/**
 * NodePalette — collapsible sidebar listing draggable trigger types and agents.
 *
 * Users drag items from the palette onto the WorkflowCanvas to add nodes.
 * HTML5 drag-and-drop is used; the canvas onDrop handler reads the transfer
 * data to create the appropriate node.
 *
 * Sections:
 * - Trigger Sources — grouped by TriggerCategory, all 14 types
 * - Agents         — live list of agents from the API (running agents only draggable)
 */

import {
	Activity,
	Bot,
	CheckCircle,
	ChevronLeft,
	ChevronRight,
	Clock,
	GitFork,
	GitMerge,
	GitPullRequest,
	Hand,
	ListOrdered,
	MessageCircle,
	Moon,
	Search,
	SquareKanban,
	Timer,
	Webhook,
	type LucideIcon,
} from "lucide-react";
import { useMemo, useState } from "react";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";
import type {
	Agent,
	TriggerCategory,
	TriggerType,
} from "@/types/orchestrator";
import { getTriggerCategory, getTriggerLabel } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Drag transfer data
// ---------------------------------------------------------------------------

/** Data set on drag events from palette items */
export type PaletteDragData =
	| { type: "trigger"; triggerType: TriggerType }
	| { type: "agent"; agentId: string; agentName: string };

export const PALETTE_DRAG_KEY = "application/agentd-palette";

export function encodeDragData(data: PaletteDragData): string {
	return JSON.stringify(data);
}

export function decodeDragData(raw: string): PaletteDragData | null {
	try {
		return JSON.parse(raw) as PaletteDragData;
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// Icon map (mirrors TriggerNode.tsx)
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

const CATEGORY_ACCENT: Record<TriggerCategory, string> = {
	external: "text-blue-600 dark:text-blue-400",
	schedule: "text-violet-600 dark:text-violet-400",
	event: "text-amber-600 dark:text-amber-400",
	internal: "text-slate-600 dark:text-slate-400",
};

const CATEGORY_LABEL: Record<TriggerCategory, string> = {
	external: "External Sources",
	schedule: "Schedules",
	event: "Events",
	internal: "Internal",
};

// All trigger types ordered by category
const ALL_TRIGGER_TYPES: TriggerType[] = [
	"github_issues",
	"github_pull_requests",
	"linear_issues",
	"webhook",
	"cron",
	"delay",
	"agent_lifecycle",
	"agent_idle",
	"dispatch_result",
	"ask_response",
	"manual",
	"queue",
	"composite",
];

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface NodePaletteProps {
	/** Live agent list from useAgents() */
	agents: Agent[];
	/** Controlled collapsed state (optional — uses internal state if omitted) */
	collapsed?: boolean;
	onCollapsedChange?: (collapsed: boolean) => void;
}

// ---------------------------------------------------------------------------
// PaletteItem — a single draggable row
// ---------------------------------------------------------------------------

function PaletteItem({
	icon: Icon,
	label,
	iconClass,
	disabled,
	disabledReason,
	dragData,
}: {
	icon: LucideIcon;
	label: string;
	iconClass?: string;
	disabled?: boolean;
	disabledReason?: string;
	dragData: PaletteDragData;
}) {
	function handleDragStart(e: React.DragEvent) {
		e.dataTransfer.effectAllowed = "copy";
		e.dataTransfer.setData(PALETTE_DRAG_KEY, encodeDragData(dragData));
	}

	return (
		<div
			draggable={!disabled}
			onDragStart={!disabled ? handleDragStart : undefined}
			title={disabled ? disabledReason : `Drag to add ${label}`}
			data-testid={`palette-item-${dragData.type === "trigger" ? dragData.triggerType : dragData.agentId}`}
			className={[
				"flex items-center gap-2 rounded px-2 py-1.5 text-xs transition-colors",
				"select-none",
				disabled
					? "cursor-not-allowed opacity-40"
					: "cursor-grab hover:bg-th-surface-hover active:cursor-grabbing",
			].join(" ")}
		>
			<Icon
				size={13}
				className={iconClass ?? "text-th-text-muted"}
				aria-hidden="true"
			/>
			<span className="text-th-text-secondary truncate">{label}</span>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Section header
// ---------------------------------------------------------------------------

function SectionHeader({ label }: { label: string }) {
	return (
		<p className="mt-3 mb-0.5 px-2 text-[10px] font-semibold uppercase tracking-widest text-th-text-faint first:mt-1">
			{label}
		</p>
	);
}

// ---------------------------------------------------------------------------
// NodePalette
// ---------------------------------------------------------------------------

export function NodePalette({
	agents,
	collapsed: collapsedProp,
	onCollapsedChange,
}: NodePaletteProps) {
	const [internalCollapsed, setInternalCollapsed] = useState(false);
	const [search, setSearch] = useState("");

	const collapsed = collapsedProp ?? internalCollapsed;

	function toggleCollapsed() {
		const next = !collapsed;
		setInternalCollapsed(next);
		onCollapsedChange?.(next);
	}

	const searchLower = search.toLowerCase();

	// Group trigger types by category, filtered by search
	const groupedTriggers = useMemo(() => {
		const categories: TriggerCategory[] = [
			"external",
			"schedule",
			"event",
			"internal",
		];
		return categories.map((cat) => ({
			category: cat,
			types: ALL_TRIGGER_TYPES.filter(
				(t) =>
					getTriggerCategory(t) === cat &&
					getTriggerLabel(t).toLowerCase().includes(searchLower),
			),
		})).filter((g) => g.types.length > 0);
	}, [searchLower]);

	// Filter agents by search
	const filteredAgents = useMemo(
		() =>
			agents.filter((a) => a.name.toLowerCase().includes(searchLower)),
		[agents, searchLower],
	);

	const hasResults =
		groupedTriggers.length > 0 || filteredAgents.length > 0;

	if (collapsed) {
		return (
			<div
				data-testid="node-palette"
				data-collapsed="true"
				className="flex flex-col items-center w-10 border-r border-th-border bg-th-surface py-2 gap-1 flex-shrink-0"
			>
				<button
					type="button"
					onClick={toggleCollapsed}
					aria-label="Expand node palette"
					className="rounded p-1 text-th-text-muted hover:text-th-text hover:bg-th-surface-hover"
				>
					<ChevronRight size={16} />
				</button>
			</div>
		);
	}

	return (
		<div
			data-testid="node-palette"
			data-collapsed="false"
			className="flex flex-col w-52 border-r border-th-border bg-th-surface flex-shrink-0 overflow-hidden"
		>
			{/* Header */}
			<div className="flex items-center justify-between px-3 py-2 border-b border-th-border">
				<span className="text-xs font-semibold text-th-text-secondary">
					Palette
				</span>
				<button
					type="button"
					onClick={toggleCollapsed}
					aria-label="Collapse node palette"
					className="rounded p-0.5 text-th-text-muted hover:text-th-text hover:bg-th-surface-hover"
				>
					<ChevronLeft size={14} />
				</button>
			</div>

			{/* Search */}
			<div className="px-2 pt-2 pb-1">
				<div className="relative">
					<Search
						size={11}
						className="absolute left-2 top-1/2 -translate-y-1/2 text-th-text-faint"
						aria-hidden="true"
					/>
					<input
						type="search"
						placeholder="Filter…"
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						data-testid="palette-search"
						className="w-full rounded border border-th-border-input bg-th-input pl-6 pr-2 py-1 text-xs text-th-text placeholder:text-th-text-faint focus:outline-none focus:ring-1 focus:ring-th-focus-ring"
					/>
				</div>
			</div>

			{/* Items */}
			<div className="flex-1 overflow-y-auto px-1 pb-3">
				{!hasResults && (
					<p className="mt-4 text-center text-xs text-th-text-faint">
						No results
					</p>
				)}

				{/* Trigger sections */}
				{groupedTriggers.map(({ category, types }) => (
					<div key={category}>
						<SectionHeader label={CATEGORY_LABEL[category]} />
						{types.map((triggerType) => {
							const Icon = TRIGGER_ICONS[triggerType] ?? Hand;
							return (
								<PaletteItem
									key={triggerType}
									icon={Icon}
									label={getTriggerLabel(triggerType)}
									iconClass={CATEGORY_ACCENT[category]}
									dragData={{ type: "trigger", triggerType }}
								/>
							);
						})}
					</div>
				))}

				{/* Agents section */}
				{filteredAgents.length > 0 && (
					<div>
						<SectionHeader label="Agents" />
						{filteredAgents.map((agent) => (
							<div key={agent.id} className="flex items-center gap-1">
								<PaletteItem
									icon={Bot}
									label={agent.name}
									iconClass="text-th-text-muted"
									disabled={agent.status !== "running"}
									disabledReason={
										agent.status !== "running"
											? `Agent is ${agent.status} — only running agents can be added`
											: undefined
									}
									dragData={{
										type: "agent",
										agentId: agent.id,
										agentName: agent.name,
									}}
								/>
								<AgentStatusBadge
									status={agent.status}
									variant="dot"
									className="ml-auto mr-1 flex-shrink-0"
								/>
							</div>
						))}
					</div>
				)}
			</div>
		</div>
	);
}

export default NodePalette;
