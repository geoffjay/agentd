/**
 * autoLayout — deterministic left-to-right layout for workflow graphs.
 *
 * Positions trigger nodes on the left (rank 0) and agent nodes on the right
 * (rank 1), grouping triggers that connect to the same agent so the agent
 * sits centered beside its triggers.  Unconnected triggers are stacked below
 * all connected groups.
 *
 * No external dependency — the workflow graph is always shallow (rank 0 →
 * rank 1) so a topological sort is not needed.
 */

import type { Edge, Node } from "@xyflow/react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AutoLayoutOptions {
	/**
	 * Layout direction.
	 * - "LR" (default): triggers left, agents right
	 * - "TB": triggers top, agents bottom
	 */
	direction?: "LR" | "TB";
	/**
	 * Center-to-center spacing between nodes in the same rank column.
	 * Default: 120
	 */
	nodeSpacing?: number;
	/**
	 * Distance between the trigger column and the agent column.
	 * Default: 300
	 */
	rankSpacing?: number;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TRIGGER_ORIGIN_LR = 80; // trigger column x offset in LR mode
const TRIGGER_ORIGIN_TB = 80; // trigger row y offset in TB mode

// ---------------------------------------------------------------------------
// autoLayout
// ---------------------------------------------------------------------------

/**
 * Returns a new array of nodes with updated `position` values.
 * Nodes that are neither "trigger" nor "agent" type are passed through
 * with their existing positions.
 */
export function autoLayout(
	nodes: Node[],
	edges: Edge[],
	options?: AutoLayoutOptions,
): Node[] {
	const { direction = "LR", nodeSpacing = 120, rankSpacing = 300 } =
		options ?? {};

	if (nodes.length === 0) return nodes;

	const triggerNodes = nodes.filter((n) => n.type === "trigger");
	const agentNodes = nodes.filter((n) => n.type === "agent");
	const otherNodes = nodes.filter(
		(n) => n.type !== "trigger" && n.type !== "agent",
	);

	// Build agent → [trigger node ids] map (preserves insertion order for
	// deterministic layout — the order triggers appear in `nodes` is used)
	const agentTriggers = new Map<string, string[]>();
	for (const agent of agentNodes) {
		agentTriggers.set(agent.id, []);
	}
	for (const edge of edges) {
		const src = nodes.find((n) => n.id === edge.source);
		const tgt = nodes.find((n) => n.id === edge.target);
		if (src?.type === "trigger" && tgt?.type === "agent") {
			const list = agentTriggers.get(edge.target);
			if (list && !list.includes(edge.source)) {
				list.push(edge.source);
			}
		}
	}

	const placedTriggerIds = new Set(
		[...agentTriggers.values()].flat(),
	);

	const positions = new Map<string, { x: number; y: number }>();

	if (direction === "LR") {
		_layoutLR(
			agentNodes,
			triggerNodes,
			agentTriggers,
			placedTriggerIds,
			positions,
			nodeSpacing,
			rankSpacing,
		);
	} else {
		_layoutTB(
			agentNodes,
			triggerNodes,
			agentTriggers,
			placedTriggerIds,
			positions,
			nodeSpacing,
			rankSpacing,
		);
	}

	// Pass other node types through unchanged
	return nodes.map((node) => {
		if (otherNodes.includes(node)) return node;
		const pos = positions.get(node.id);
		return pos ? { ...node, position: pos } : node;
	});
}

// ---------------------------------------------------------------------------
// LR layout (left-to-right): triggers left, agents right
// ---------------------------------------------------------------------------

function _layoutLR(
	agentNodes: Node[],
	triggerNodes: Node[],
	agentTriggers: Map<string, string[]>,
	placedTriggerIds: Set<string>,
	positions: Map<string, { x: number; y: number }>,
	nodeSpacing: number,
	rankSpacing: number,
): void {
	const TRIGGER_X = TRIGGER_ORIGIN_LR;
	const AGENT_X = TRIGGER_X + rankSpacing;

	// "slot" is the unit of vertical space; each slot = nodeSpacing px
	let nextSlot = 0;

	for (const agent of agentNodes) {
		const tIds = agentTriggers.get(agent.id) ?? [];
		const groupSize = Math.max(tIds.length, 1);
		const groupStart = nextSlot;

		// Position each trigger in this group
		for (let i = 0; i < tIds.length; i++) {
			positions.set(tIds[i], {
				x: TRIGGER_X,
				y: (groupStart + i) * nodeSpacing,
			});
		}

		// Center the agent vertically on its trigger group
		const agentCenterSlot = groupStart + (groupSize - 1) / 2;
		positions.set(agent.id, {
			x: AGENT_X,
			y: agentCenterSlot * nodeSpacing,
		});

		// Advance past the group, adding one blank slot as a gap
		nextSlot = groupStart + groupSize + 1;
	}

	// Unconnected triggers stacked below everything
	for (const trigger of triggerNodes) {
		if (!placedTriggerIds.has(trigger.id)) {
			positions.set(trigger.id, {
				x: TRIGGER_X,
				y: nextSlot * nodeSpacing,
			});
			nextSlot++;
		}
	}
}

// ---------------------------------------------------------------------------
// TB layout (top-to-bottom): triggers top, agents bottom
// ---------------------------------------------------------------------------

function _layoutTB(
	agentNodes: Node[],
	triggerNodes: Node[],
	agentTriggers: Map<string, string[]>,
	placedTriggerIds: Set<string>,
	positions: Map<string, { x: number; y: number }>,
	nodeSpacing: number,
	rankSpacing: number,
): void {
	const TRIGGER_Y = TRIGGER_ORIGIN_TB;
	const AGENT_Y = TRIGGER_Y + rankSpacing;

	let nextSlot = 0;

	for (const agent of agentNodes) {
		const tIds = agentTriggers.get(agent.id) ?? [];
		const groupSize = Math.max(tIds.length, 1);
		const groupStart = nextSlot;

		for (let i = 0; i < tIds.length; i++) {
			positions.set(tIds[i], {
				x: (groupStart + i) * nodeSpacing,
				y: TRIGGER_Y,
			});
		}

		const agentCenterSlot = groupStart + (groupSize - 1) / 2;
		positions.set(agent.id, {
			x: agentCenterSlot * nodeSpacing,
			y: AGENT_Y,
		});

		nextSlot = groupStart + groupSize + 1;
	}

	// Unconnected triggers to the right of everything
	for (const trigger of triggerNodes) {
		if (!placedTriggerIds.has(trigger.id)) {
			positions.set(trigger.id, {
				x: (nextSlot * nodeSpacing) + TRIGGER_ORIGIN_TB,
				y: TRIGGER_Y,
			});
			nextSlot++;
		}
	}
}
