/**
 * Workflow canvas serialization layer.
 *
 * Converts between React Flow graph state (nodes + edges) and the workflow
 * REST API format (CreateWorkflowRequest / Workflow).
 *
 * Data model mapping:
 *
 *   Canvas Graph                          REST API
 *   ────────────────────────────────────  ────────────────────────
 *   TriggerNode ─PromptEdge─> AgentNode = WorkflowConfig {
 *     triggerConfig                           trigger_config,
 *                 promptTemplate              prompt_template,
 *                 pollIntervalSecs            poll_interval_secs,
 *                              agentId        agent_id,
 *                                             name,
 *                                             enabled,
 *                                             tool_policy,
 *                                          }
 */

import type { Edge, Node } from "@xyflow/react";
import type {
	Agent,
	CreateWorkflowRequest,
	TriggerConfig,
	ToolPolicy,
	Workflow,
} from "@/types/orchestrator";
import { getTriggerCategory, getTriggerLabel } from "@/types/orchestrator";
import type { AgentNodeData } from "./nodes/AgentNode";
import type { TriggerNodeData } from "./nodes/TriggerNode";
import type { PromptEdgeData } from "./edges/PromptEdge";

// ---------------------------------------------------------------------------
// Canvas layout (stored separately from the workflow API)
// ---------------------------------------------------------------------------

export interface CanvasLayout {
	/** nodeId -> { x, y } position */
	nodes: Record<string, { x: number; y: number }>;
	viewport: { x: number; y: number; zoom: number };
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

export type SerializationErrorType =
	| "unconnected_trigger"
	| "unconnected_agent"
	| "missing_config"
	| "invalid_edge";

export interface SerializationError {
	type: SerializationErrorType;
	nodeId?: string;
	edgeId?: string;
	message: string;
}

/**
 * Validate a React Flow graph before serialization.
 *
 * Returns an array of errors; an empty array means the graph is valid.
 */
export function validateGraph(
	nodes: Node[],
	edges: Edge[],
): SerializationError[] {
	const errors: SerializationError[] = [];

	const triggerNodes = nodes.filter((n) => n.type === "trigger");
	const agentNodes = nodes.filter((n) => n.type === "agent");

	// Build sets of connected node IDs
	const connectedSources = new Set(edges.map((e) => e.source));
	const connectedTargets = new Set(edges.map((e) => e.target));

	// Every trigger node must have at least one outgoing edge
	for (const node of triggerNodes) {
		if (!connectedSources.has(node.id)) {
			errors.push({
				type: "unconnected_trigger",
				nodeId: node.id,
				message: `Trigger node "${node.id}" is not connected to any agent`,
			});
		}

		// Validate required trigger config fields
		const data = node.data as TriggerNodeData;
		if (!data?.triggerConfig) {
			errors.push({
				type: "missing_config",
				nodeId: node.id,
				message: `Trigger node "${node.id}" is missing triggerConfig`,
			});
		} else {
			const cfg = data.triggerConfig;
			if (
				(cfg.type === "github_issues" || cfg.type === "github_pull_requests") &&
				(!cfg.owner || !cfg.repo)
			) {
				errors.push({
					type: "missing_config",
					nodeId: node.id,
					message: `GitHub trigger "${node.id}" requires owner and repo`,
				});
			}
			if (cfg.type === "cron" && !cfg.expression) {
				errors.push({
					type: "missing_config",
					nodeId: node.id,
					message: `Cron trigger "${node.id}" requires an expression`,
				});
			}
			if (cfg.type === "queue" && !cfg.queue_name) {
				errors.push({
					type: "missing_config",
					nodeId: node.id,
					message: `Queue trigger "${node.id}" requires a queue_name`,
				});
			}
		}
	}

	// Every agent node must have at least one incoming edge
	for (const node of agentNodes) {
		if (!connectedTargets.has(node.id)) {
			errors.push({
				type: "unconnected_agent",
				nodeId: node.id,
				message: `Agent node "${node.id}" has no incoming trigger connections`,
			});
		}

		const data = node.data as AgentNodeData;
		if (!data?.agentId) {
			errors.push({
				type: "missing_config",
				nodeId: node.id,
				message: `Agent node "${node.id}" is missing agentId`,
			});
		}
	}

	// Validate edges connect trigger -> agent
	for (const edge of edges) {
		const srcNode = nodes.find((n) => n.id === edge.source);
		const tgtNode = nodes.find((n) => n.id === edge.target);
		if (!srcNode || !tgtNode) {
			errors.push({
				type: "invalid_edge",
				edgeId: edge.id,
				message: `Edge "${edge.id}" references a missing node`,
			});
			continue;
		}
		if (srcNode.type !== "trigger" || tgtNode.type !== "agent") {
			errors.push({
				type: "invalid_edge",
				edgeId: edge.id,
				message: `Edge "${edge.id}" must connect a trigger node to an agent node`,
			});
		}
	}

	return errors;
}

// ---------------------------------------------------------------------------
// Graph → API
// ---------------------------------------------------------------------------

/**
 * Convert React Flow graph state into an array of workflow API requests.
 *
 * Each trigger→agent edge becomes one `CreateWorkflowRequest`.
 *
 * @param nodes - React Flow node list (trigger + agent nodes)
 * @param edges - React Flow edge list (prompt edges)
 * @param defaultPolicy - tool policy applied when no override is present
 * @throws {Error} when the graph contains validation errors
 */
export function graphToWorkflows(
	nodes: Node[],
	edges: Edge[],
	defaultPolicy: ToolPolicy = { mode: "allow_all" },
): CreateWorkflowRequest[] {
	const errors = validateGraph(nodes, edges);
	if (errors.length > 0) {
		throw new Error(
			`Graph validation failed:\n${errors.map((e) => `  • ${e.message}`).join("\n")}`,
		);
	}

	const nodeMap = new Map(nodes.map((n) => [n.id, n]));
	const requests: CreateWorkflowRequest[] = [];

	for (const edge of edges) {
		const triggerNode = nodeMap.get(edge.source);
		const agentNode = nodeMap.get(edge.target);

		if (
			!triggerNode ||
			triggerNode.type !== "trigger" ||
			!agentNode ||
			agentNode.type !== "agent"
		) {
			continue;
		}

		const triggerData = triggerNode.data as TriggerNodeData;
		const agentData = agentNode.data as AgentNodeData;
		const edgeData = (edge.data ?? {}) as Partial<PromptEdgeData>;

		const triggerConfig = triggerData.triggerConfig as TriggerConfig;
		const agentId = agentData.agentId;
		const promptTemplate = edgeData.promptTemplate ?? "";
		const pollIntervalSecs = edgeData.pollIntervalSecs ?? 300;
		const enabled = triggerData.enabled ?? true;

		// Derive workflow name from trigger type and agent name
		const triggerSlug = triggerConfig.type.replace(/_/g, "-");
		const agentSlug = agentData.name
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/^-|-$/g, "");
		const name = `${triggerSlug}-to-${agentSlug}`;

		requests.push({
			name,
			agent_id: agentId,
			trigger_config: triggerConfig,
			prompt_template: promptTemplate,
			poll_interval_secs: pollIntervalSecs,
			enabled,
			tool_policy: agentData.toolPolicy ?? defaultPolicy,
		});
	}

	return requests;
}

// ---------------------------------------------------------------------------
// API → Graph
// ---------------------------------------------------------------------------

/** Default column spacing for auto-generated layout */
const LAYOUT_TRIGGER_X = 80;
const LAYOUT_AGENT_X = 380;
const LAYOUT_ROW_HEIGHT = 120;
const LAYOUT_START_Y = 60;

/**
 * Convert a set of workflow API responses into React Flow graph state.
 *
 * Agent nodes are deduplicated: if multiple workflows share the same
 * `agent_id`, they all connect to a single agent node.
 *
 * @param workflows - Workflow responses from the API
 * @param agents    - Full agent list (used to populate node data)
 * @param layout    - Optional saved layout; auto-generated when absent
 */
export function workflowsToGraph(
	workflows: Workflow[],
	agents: Agent[],
	layout?: CanvasLayout,
): { nodes: Node[]; edges: Edge[] } {
	const agentMap = new Map(agents.map((a) => [a.id, a]));

	const triggerNodes: Node<TriggerNodeData>[] = [];
	const agentNodeMap = new Map<string, Node<AgentNodeData>>();
	const edges: Edge<PromptEdgeData>[] = [];

	// Track row index for auto-layout
	let rowIndex = 0;

	for (const wf of workflows) {
		// ── Trigger node ──────────────────────────────────────────────
		const triggerId = `trigger-${wf.id}`;
		const triggerConfig = wf.trigger_config;
		const category = getTriggerCategory(triggerConfig.type);
		const savedTriggerPos = layout?.nodes[triggerId];

		triggerNodes.push({
			id: triggerId,
			type: "trigger",
			position: savedTriggerPos ?? {
				x: LAYOUT_TRIGGER_X,
				y: LAYOUT_START_Y + rowIndex * LAYOUT_ROW_HEIGHT,
			},
			data: {
				triggerConfig,
				label: getTriggerLabel(triggerConfig.type),
				category,
				enabled: wf.enabled,
			},
		});

		// ── Agent node (deduplicate) ──────────────────────────────────
		const agentId = wf.agent_id;
		const agentNodeId = `agent-${agentId}`;

		if (!agentNodeMap.has(agentNodeId)) {
			const agent = agentMap.get(agentId);
			const savedAgentPos = layout?.nodes[agentNodeId];

			agentNodeMap.set(agentNodeId, {
				id: agentNodeId,
				type: "agent",
				position: savedAgentPos ?? {
					x: LAYOUT_AGENT_X,
					// Centre agent vertically on first workflow that references it
					y: LAYOUT_START_Y + rowIndex * LAYOUT_ROW_HEIGHT,
				},
				data: {
					agentId,
					name: agent?.name ?? `Agent (${agentId.slice(0, 8)})`,
					status: agent?.status ?? "stopped",
					model: agent?.config?.model,
					toolPolicy: wf.tool_policy,
				},
			});
		}

		// ── Edge ──────────────────────────────────────────────────────
		const edgeId = `edge-${wf.id}`;
		edges.push({
			id: edgeId,
			source: triggerId,
			target: agentNodeId,
			type: "prompt",
			data: {
				promptTemplate: wf.prompt_template,
				pollIntervalSecs: wf.poll_interval_secs,
				enabled: wf.enabled,
			},
		});

		rowIndex++;
	}

	return {
		nodes: [...triggerNodes, ...agentNodeMap.values()],
		edges,
	};
}

// ---------------------------------------------------------------------------
// Layout persistence helpers
// ---------------------------------------------------------------------------

/**
 * Derive a stable storage key from a set of workflow IDs.
 * Used to namespace localStorage layout data per canvas composition.
 */
export function layoutStorageKey(workflowIds: string[]): string {
	const sorted = [...workflowIds].sort().join(",");
	// Simple djb2-style hash for a stable short key
	let hash = 5381;
	for (let i = 0; i < sorted.length; i++) {
		hash = ((hash << 5) + hash) ^ sorted.charCodeAt(i);
	}
	return `wf-layout-${(hash >>> 0).toString(16)}`;
}

/** Persist canvas layout to localStorage */
export function saveLayout(
	workflowIds: string[],
	layout: CanvasLayout,
): void {
	try {
		const key = layoutStorageKey(workflowIds);
		localStorage.setItem(key, JSON.stringify(layout));
	} catch {
		// localStorage may be unavailable (private browsing, storage quota)
	}
}

/** Load canvas layout from localStorage; returns undefined when not found */
export function loadLayout(workflowIds: string[]): CanvasLayout | undefined {
	try {
		const key = layoutStorageKey(workflowIds);
		const raw = localStorage.getItem(key);
		if (!raw) return undefined;
		return JSON.parse(raw) as CanvasLayout;
	} catch {
		return undefined;
	}
}
