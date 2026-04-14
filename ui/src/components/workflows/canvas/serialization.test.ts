/**
 * Serialization tests.
 *
 * Covers:
 * - graphToWorkflows: converts graph state to CreateWorkflowRequest[]
 * - workflowsToGraph: converts Workflow[] to React Flow nodes/edges
 * - validateGraph: catches unconnected nodes, missing config, invalid edges
 * - Round-trip: graph → API → graph preserves all workflow data
 * - Edge cases: empty graph, single node, multi-workflow with shared agent
 * - Layout helpers: saveLayout / loadLayout / layoutStorageKey
 */

import { describe, expect, it, beforeEach, afterEach } from "vitest";
import type { Edge, Node } from "@xyflow/react";
import type { Agent, Workflow } from "@/types/orchestrator";
import type { AgentNodeData } from "./nodes/AgentNode";
import type { TriggerNodeData } from "./nodes/TriggerNode";
import type { PromptEdgeData } from "./edges/PromptEdge";
import {
	graphToWorkflows,
	layoutStorageKey,
	loadLayout,
	saveLayout,
	validateGraph,
	workflowsToGraph,
} from "./serialization";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeTriggerNode(
	id: string,
	overrides: Partial<TriggerNodeData> = {},
): Node<TriggerNodeData> {
	return {
		id,
		type: "trigger",
		position: { x: 0, y: 0 },
		data: {
			triggerConfig: {
				type: "github_issues",
				owner: "acme",
				repo: "myrepo",
				labels: [],
				state: "open",
			},
			category: "external",
			enabled: true,
			...overrides,
		},
	};
}

function makeAgentNode(
	id: string,
	overrides: Partial<AgentNodeData> = {},
): Node<AgentNodeData> {
	return {
		id,
		type: "agent",
		position: { x: 300, y: 0 },
		data: {
			agentId: `agent-${id}`,
			name: "Test Agent",
			status: "running",
			toolPolicy: { mode: "allow_all" },
			...overrides,
		},
	};
}

function makeEdge(
	id: string,
	source: string,
	target: string,
	overrides: Partial<PromptEdgeData> = {},
): Edge<PromptEdgeData> {
	return {
		id,
		source,
		target,
		type: "prompt",
		data: {
			promptTemplate: "Fix: {{title}}",
			pollIntervalSecs: 300,
			enabled: true,
			...overrides,
		},
	};
}

function makeWorkflow(id: string, agentId = "agent-1"): Workflow {
	return {
		id,
		name: `wf-${id}`,
		agent_id: agentId,
		trigger_config: {
			type: "github_issues",
			owner: "acme",
			repo: "myrepo",
			labels: ["bug"],
			state: "open",
		},
		prompt_template: "Fix: {{title}}",
		poll_interval_secs: 300,
		enabled: true,
		tool_policy: { mode: "allow_all" },
		created_at: "2024-01-01T00:00:00Z",
		updated_at: "2024-01-01T00:00:00Z",
	};
}

function makeAgent(id: string, name = "Test Agent"): Agent {
	return {
		id,
		name,
		status: "running",
		config: {
			working_dir: "/tmp",
			shell: "/bin/sh",
			interactive: false,
			tool_policy: { mode: "allow_all" },
			model: "claude-sonnet",
		},
		created_at: "2024-01-01T00:00:00Z",
		updated_at: "2024-01-01T00:00:00Z",
	};
}

// ---------------------------------------------------------------------------
// validateGraph
// ---------------------------------------------------------------------------

describe("validateGraph", () => {
	it("returns empty array for a valid single-workflow graph", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges = [makeEdge("e1", "t1", "a1")];
		expect(validateGraph(nodes, edges)).toHaveLength(0);
	});

	it("returns empty array for an empty graph", () => {
		expect(validateGraph([], [])).toHaveLength(0);
	});

	it("reports unconnected trigger node", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges: Edge[] = []; // no connection
		const errors = validateGraph(nodes, edges);
		expect(errors.some((e) => e.type === "unconnected_trigger")).toBe(true);
	});

	it("reports unconnected agent node", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges: Edge[] = []; // no connection
		const errors = validateGraph(nodes, edges);
		expect(errors.some((e) => e.type === "unconnected_agent")).toBe(true);
	});

	it("reports missing triggerConfig", () => {
		const nodes = [
			{
				id: "t1",
				type: "trigger",
				position: { x: 0, y: 0 },
				data: {} as TriggerNodeData,
			},
			makeAgentNode("a1"),
		];
		const edges = [makeEdge("e1", "t1", "a1")];
		const errors = validateGraph(nodes, edges);
		expect(errors.some((e) => e.type === "missing_config")).toBe(true);
	});

	it("reports missing owner/repo for github_issues", () => {
		const nodes = [
			makeTriggerNode("t1", {
				triggerConfig: {
					type: "github_issues",
					owner: "",
					repo: "",
					labels: [],
					state: "open",
				},
			}),
			makeAgentNode("a1"),
		];
		const edges = [makeEdge("e1", "t1", "a1")];
		const errors = validateGraph(nodes, edges);
		expect(errors.some((e) => e.type === "missing_config")).toBe(true);
	});

	it("reports invalid edge (agent → trigger reversed)", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		// Reversed direction
		const edges = [makeEdge("e1", "a1", "t1")];
		const errors = validateGraph(nodes, edges);
		expect(errors.some((e) => e.type === "invalid_edge")).toBe(true);
	});

	it("returns no errors for multi-workflow graph with shared agent", () => {
		const nodes = [
			makeTriggerNode("t1"),
			makeTriggerNode("t2"),
			makeAgentNode("a1", { agentId: "agent-shared" }),
		];
		const edges = [
			makeEdge("e1", "t1", "a1"),
			makeEdge("e2", "t2", "a1"),
		];
		expect(validateGraph(nodes, edges)).toHaveLength(0);
	});
});

// ---------------------------------------------------------------------------
// graphToWorkflows
// ---------------------------------------------------------------------------

describe("graphToWorkflows", () => {
	it("converts a single trigger→agent edge to one request", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges = [makeEdge("e1", "t1", "a1")];
		const requests = graphToWorkflows(nodes, edges);
		expect(requests).toHaveLength(1);
	});

	it("maps trigger_config from trigger node data", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges = [makeEdge("e1", "t1", "a1")];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.trigger_config).toMatchObject({
			type: "github_issues",
			owner: "acme",
			repo: "myrepo",
		});
	});

	it("maps agent_id from agent node data", () => {
		const nodes = [
			makeTriggerNode("t1"),
			makeAgentNode("a1", { agentId: "uuid-1234" }),
		];
		const edges = [makeEdge("e1", "t1", "a1")];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.agent_id).toBe("uuid-1234");
	});

	it("maps prompt_template from edge data", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges = [
			makeEdge("e1", "t1", "a1", { promptTemplate: "My custom prompt" }),
		];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.prompt_template).toBe("My custom prompt");
	});

	it("maps poll_interval_secs from edge data", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		const edges = [makeEdge("e1", "t1", "a1", { pollIntervalSecs: 900 })];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.poll_interval_secs).toBe(900);
	});

	it("maps enabled from trigger node data", () => {
		const nodes = [
			makeTriggerNode("t1", { enabled: false }),
			makeAgentNode("a1"),
		];
		const edges = [makeEdge("e1", "t1", "a1")];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.enabled).toBe(false);
	});

	it("derives workflow name from trigger type and agent name", () => {
		const nodes = [
			makeTriggerNode("t1"),
			makeAgentNode("a1", { agentId: "uuid", name: "My Worker Agent" }),
		];
		const edges = [makeEdge("e1", "t1", "a1")];
		const [req] = graphToWorkflows(nodes, edges);
		expect(req.name).toBe("github-issues-to-my-worker-agent");
	});

	it("produces one request per edge for multi-workflow graph", () => {
		const nodes = [
			makeTriggerNode("t1"),
			makeTriggerNode("t2"),
			makeAgentNode("a1"),
		];
		const edges = [
			makeEdge("e1", "t1", "a1"),
			makeEdge("e2", "t2", "a1"),
		];
		expect(graphToWorkflows(nodes, edges)).toHaveLength(2);
	});

	it("throws on invalid graph (unconnected trigger)", () => {
		const nodes = [makeTriggerNode("t1"), makeAgentNode("a1")];
		expect(() => graphToWorkflows(nodes, [])).toThrow(/validation failed/i);
	});

	it("returns empty array for empty graph", () => {
		expect(graphToWorkflows([], [])).toHaveLength(0);
	});
});

// ---------------------------------------------------------------------------
// workflowsToGraph
// ---------------------------------------------------------------------------

describe("workflowsToGraph", () => {
	it("creates one trigger node per workflow", () => {
		const workflows = [makeWorkflow("w1"), makeWorkflow("w2")];
		const { nodes } = workflowsToGraph(workflows, []);
		const triggers = nodes.filter((n) => n.type === "trigger");
		expect(triggers).toHaveLength(2);
	});

	it("deduplicates agent nodes for shared agent_id", () => {
		const workflows = [
			makeWorkflow("w1", "agent-1"),
			makeWorkflow("w2", "agent-1"),
		];
		const { nodes } = workflowsToGraph(workflows, []);
		const agentNodes = nodes.filter((n) => n.type === "agent");
		expect(agentNodes).toHaveLength(1);
	});

	it("creates separate agent nodes for different agent_ids", () => {
		const workflows = [
			makeWorkflow("w1", "agent-1"),
			makeWorkflow("w2", "agent-2"),
		];
		const { nodes } = workflowsToGraph(workflows, []);
		const agentNodes = nodes.filter((n) => n.type === "agent");
		expect(agentNodes).toHaveLength(2);
	});

	it("creates one edge per workflow", () => {
		const workflows = [makeWorkflow("w1"), makeWorkflow("w2")];
		const { edges } = workflowsToGraph(workflows, []);
		expect(edges).toHaveLength(2);
	});

	it("populates trigger node with triggerConfig", () => {
		const wf = makeWorkflow("w1");
		const { nodes } = workflowsToGraph([wf], []);
		const trigger = nodes.find((n) => n.type === "trigger");
		expect(
			(trigger?.data as TriggerNodeData).triggerConfig,
		).toMatchObject({ type: "github_issues" });
	});

	it("populates agent node with name from agent list", () => {
		const wf = makeWorkflow("w1", "agent-42");
		const agent = makeAgent("agent-42", "Smart Worker");
		const { nodes } = workflowsToGraph([wf], [agent]);
		const agentNode = nodes.find((n) => n.type === "agent");
		expect((agentNode?.data as AgentNodeData).name).toBe("Smart Worker");
	});

	it("uses fallback name when agent is not in list", () => {
		const wf = makeWorkflow("w1", "unknown-agent-id");
		const { nodes } = workflowsToGraph([wf], []);
		const agentNode = nodes.find((n) => n.type === "agent");
		expect(
			(agentNode?.data as AgentNodeData).name,
		).toMatch(/Agent \(unknown-/);
	});

	it("populates edge with promptTemplate from workflow", () => {
		const wf = { ...makeWorkflow("w1"), prompt_template: "My prompt" };
		const { edges } = workflowsToGraph([wf], []);
		expect((edges[0].data as PromptEdgeData).promptTemplate).toBe("My prompt");
	});

	it("populates edge with pollIntervalSecs from workflow", () => {
		const wf = { ...makeWorkflow("w1"), poll_interval_secs: 600 };
		const { edges } = workflowsToGraph([wf], []);
		expect((edges[0].data as PromptEdgeData).pollIntervalSecs).toBe(600);
	});

	it("returns empty nodes and edges for empty workflow list", () => {
		const { nodes, edges } = workflowsToGraph([], []);
		expect(nodes).toHaveLength(0);
		expect(edges).toHaveLength(0);
	});

	it("applies saved layout positions when provided", () => {
		const wf = makeWorkflow("w1", "agent-1");
		const layout = {
			nodes: {
				"trigger-w1": { x: 999, y: 888 },
				"agent-agent-1": { x: 777, y: 666 },
			},
			viewport: { x: 0, y: 0, zoom: 1 },
		};
		const { nodes } = workflowsToGraph([wf], [], layout);
		const trigger = nodes.find((n) => n.id === "trigger-w1");
		const agentNode = nodes.find((n) => n.id === "agent-agent-1");
		expect(trigger?.position).toEqual({ x: 999, y: 888 });
		expect(agentNode?.position).toEqual({ x: 777, y: 666 });
	});
});

// ---------------------------------------------------------------------------
// Round-trip test
// ---------------------------------------------------------------------------

describe("round-trip: graph → API → graph", () => {
	it("preserves trigger config, agent id, prompt, and interval", () => {
		// Build a simple graph
		const origNodes = [
			makeTriggerNode("t1", {
				triggerConfig: {
					type: "cron",
					expression: "*/5 * * * *",
				},
				enabled: true,
			}),
			makeAgentNode("a1", {
				agentId: "uuid-abc",
				name: "My Agent",
				toolPolicy: { mode: "require_approval" },
			}),
		];
		const origEdges = [
			makeEdge("e1", "t1", "a1", {
				promptTemplate: "Run the cron task: {{title}}",
				pollIntervalSecs: 60,
			}),
		];

		// Serialize to API requests
		const requests = graphToWorkflows(origNodes, origEdges);
		expect(requests).toHaveLength(1);
		const req = requests[0];

		// Simulate API round-trip: build a Workflow from the request
		const workflow: Workflow = {
			id: "wf-new",
			name: req.name,
			agent_id: req.agent_id,
			trigger_config: req.trigger_config,
			prompt_template: req.prompt_template,
			poll_interval_secs: req.poll_interval_secs,
			enabled: req.enabled,
			tool_policy: req.tool_policy,
			created_at: "2024-01-01T00:00:00Z",
			updated_at: "2024-01-01T00:00:00Z",
		};

		const agent = makeAgent("uuid-abc", "My Agent");
		const { nodes: outNodes, edges: outEdges } = workflowsToGraph(
			[workflow],
			[agent],
		);

		// Verify round-trip fidelity
		const triggerOut = outNodes.find((n) => n.type === "trigger");
		const agentOut = outNodes.find((n) => n.type === "agent");
		const edgeOut = outEdges[0];

		expect(
			(triggerOut?.data as TriggerNodeData).triggerConfig,
		).toMatchObject({ type: "cron", expression: "*/5 * * * *" });
		expect(
			(agentOut?.data as AgentNodeData).name,
		).toBe("My Agent");
		expect(
			(edgeOut?.data as PromptEdgeData).promptTemplate,
		).toBe("Run the cron task: {{title}}");
		expect(
			(edgeOut?.data as PromptEdgeData).pollIntervalSecs,
		).toBe(60);
	});
});

// ---------------------------------------------------------------------------
// Layout persistence
// ---------------------------------------------------------------------------

describe("layoutStorageKey", () => {
	it("returns a stable key for the same workflow IDs", () => {
		const ids = ["wf-1", "wf-2", "wf-3"];
		expect(layoutStorageKey(ids)).toBe(layoutStorageKey(ids));
	});

	it("returns the same key regardless of input order", () => {
		expect(layoutStorageKey(["wf-1", "wf-2"])).toBe(
			layoutStorageKey(["wf-2", "wf-1"]),
		);
	});

	it("returns different keys for different workflow sets", () => {
		expect(layoutStorageKey(["wf-1"])).not.toBe(layoutStorageKey(["wf-2"]));
	});
});

describe("saveLayout / loadLayout", () => {
	beforeEach(() => localStorage.clear());
	afterEach(() => localStorage.clear());

	it("saves and loads a layout", () => {
		const ids = ["wf-1", "wf-2"];
		const layout = {
			nodes: { "trigger-wf-1": { x: 100, y: 200 } },
			viewport: { x: 0, y: 0, zoom: 1.5 },
		};
		saveLayout(ids, layout);
		expect(loadLayout(ids)).toEqual(layout);
	});

	it("returns undefined when no layout is saved", () => {
		expect(loadLayout(["wf-unknown"])).toBeUndefined();
	});

	it("overwrites previous layout on re-save", () => {
		const ids = ["wf-1"];
		const layout1 = {
			nodes: { "trigger-wf-1": { x: 10, y: 20 } },
			viewport: { x: 0, y: 0, zoom: 1 },
		};
		const layout2 = {
			nodes: { "trigger-wf-1": { x: 99, y: 88 } },
			viewport: { x: 5, y: 5, zoom: 2 },
		};
		saveLayout(ids, layout1);
		saveLayout(ids, layout2);
		expect(loadLayout(ids)).toEqual(layout2);
	});
});
