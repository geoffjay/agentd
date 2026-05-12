/**
 * AgentNode — custom React Flow node for workflow agent targets.
 *
 * Displays:
 * - Agent name (primary text)
 * - Status badge (running/stopped/pending/failed)
 * - Model name (if known)
 * - Tool policy summary
 * - Input handle on the left for incoming trigger connections
 *
 * Styling reflects agent status: green border for running, red for failed,
 * amber for pending, grey for stopped.
 */

import { Bot } from "lucide-react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import { AgentStatusBadge } from "@/components/agents/AgentStatusBadge";
import type { AgentStatus, ToolPolicy } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Node data interface
// ---------------------------------------------------------------------------

export interface AgentNodeData extends Record<string, unknown> {
	agentId: string;
	name: string;
	status: AgentStatus;
	model?: string;
	toolPolicy: ToolPolicy;
	onAgentChange?: (agentId: string) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function toolPolicySummary(policy: ToolPolicy): string {
	switch (policy.mode) {
		case "allow_all":
			return "Allow all tools";
		case "deny_all":
			return "Deny all tools";
		case "allow_list":
			return `${policy.tools.length} tool${policy.tools.length !== 1 ? "s" : ""} allowed`;
		case "deny_list":
			return `${policy.tools.length} tool${policy.tools.length !== 1 ? "s" : ""} denied`;
		case "require_approval":
			return "Requires approval";
		default:
			return "Custom policy";
	}
}

/** Border colour classes per status */
const STATUS_BORDER: Record<AgentStatus, string> = {
	running: "border-green-400 dark:border-green-600",
	failed: "border-red-400 dark:border-red-600",
	pending: "border-amber-400 dark:border-amber-600",
	stopped: "border-slate-300 dark:border-slate-600",
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function AgentNode({ data, selected }: NodeProps<AgentNodeData>) {
	const { name, status, model, toolPolicy } = data;
	const borderClass = STATUS_BORDER[status] ?? STATUS_BORDER.stopped;

	return (
		<div
			data-testid="agent-node"
			data-agent-id={data.agentId}
			className={[
				"relative min-w-[180px] max-w-[240px] rounded-lg border-2 px-3 py-2.5 shadow-sm transition-all",
				"bg-th-surface",
				selected
					? "border-blue-500 ring-2 ring-blue-300 dark:ring-blue-700"
					: borderClass,
			]
				.filter(Boolean)
				.join(" ")}
		>
			{/* Header: icon + name + status */}
			<div className="flex items-center gap-1.5">
				<Bot
					size={14}
					className="text-th-text-muted flex-shrink-0"
					aria-hidden="true"
				/>
				<span
					className="text-xs font-semibold text-th-text truncate flex-1"
					title={name}
				>
					{name}
				</span>
				<AgentStatusBadge status={status} variant="dot" />
			</div>

			{/* Model */}
			{model && (
				<p
					className="mt-1 text-[11px] text-th-text-muted truncate"
					title={model}
					data-testid="agent-node-model"
				>
					{model}
				</p>
			)}

			{/* Tool policy */}
			<p className="mt-0.5 text-[11px] text-th-text-faint">
				{toolPolicySummary(toolPolicy)}
			</p>

			{/* Input handle — left side */}
			<Handle
				type="target"
				position={Position.Left}
				id="in"
				className="!w-3 !h-3 !bg-th-accent !border-2 !border-white"
				data-testid="agent-node-handle-in"
			/>
		</div>
	);
}

export default AgentNode;
