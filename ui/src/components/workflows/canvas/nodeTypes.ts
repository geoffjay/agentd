/**
 * workflowNodeTypes / workflowEdgeTypes — React Flow type registries
 * for the workflow canvas.
 *
 * Import these maps and pass them as the `nodeTypes` / `edgeTypes` props
 * to <WorkflowCanvas> (or directly to <ReactFlow>) so React Flow can
 * resolve custom node/edge components by their string type key.
 */

import type { EdgeTypes, NodeTypes } from "@xyflow/react";
import { PromptEdge } from "./edges/PromptEdge";
import { AgentNode } from "./nodes/AgentNode";
import { TriggerNode } from "./nodes/TriggerNode";

export const workflowNodeTypes = {
	trigger: TriggerNode,
	agent: AgentNode,
} satisfies NodeTypes;

export const workflowEdgeTypes = {
	prompt: PromptEdge,
} satisfies EdgeTypes;

export default workflowNodeTypes;
