/**
 * workflowNodeTypes — React Flow node type registry for the workflow canvas.
 *
 * Import this map and pass it as the `nodeTypes` prop to <WorkflowCanvas>
 * (or directly to <ReactFlow>) so React Flow can resolve custom node
 * components by their string type key.
 *
 * New node types (e.g. "agent") will be added here as they are implemented.
 */

import type { NodeTypes } from "@xyflow/react";
import { TriggerNode } from "./nodes/TriggerNode";

export const workflowNodeTypes = {
	trigger: TriggerNode,
} satisfies NodeTypes;

export default workflowNodeTypes;
