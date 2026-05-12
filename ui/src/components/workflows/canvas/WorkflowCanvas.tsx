/**
 * WorkflowCanvas — base React Flow canvas for the visual workflow builder.
 *
 * Renders a ReactFlow instance with Controls, MiniMap, and Background.
 * Accepts standard React Flow props so parent components can manage
 * node/edge state and handle connection events.
 *
 * Wrap a page-level ancestor in <ReactFlowProvider> when multiple canvas
 * instances need to coexist; for single-canvas pages the provider is
 * included here.
 */

import "@xyflow/react/dist/style.css";
import {
	Background,
	BackgroundVariant,
	Controls,
	MiniMap,
	ReactFlow,
	ReactFlowProvider,
	type Edge,
	type EdgeTypes,
	type Node,
	type NodeTypes,
	type OnConnect,
	type OnEdgesChange,
	type OnNodesChange,
	type ReactFlowInstance,
} from "@xyflow/react";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface WorkflowCanvasProps {
	/** Current node list */
	nodes: Node[];
	/** Current edge list */
	edges: Edge[];
	/** Handler for node changes (move, select, remove) */
	onNodesChange: OnNodesChange;
	/** Handler for edge changes (select, remove) */
	onEdgesChange: OnEdgesChange;
	/** Handler for new connections drawn by the user */
	onConnect: OnConnect;
	/** Custom node type registry — merge with workflowNodeTypes */
	nodeTypes?: NodeTypes;
	/** Custom edge type registry — merge with workflowEdgeTypes */
	edgeTypes?: EdgeTypes;
	/** Optional CSS class applied to the outer wrapper */
	className?: string;
	/** Called with the ReactFlowInstance once the canvas is ready */
	onInit?: (instance: ReactFlowInstance) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

function Canvas({
	nodes,
	edges,
	onNodesChange,
	onEdgesChange,
	onConnect,
	nodeTypes,
	edgeTypes,
	className = "",
	onInit,
}: WorkflowCanvasProps) {
	return (
		<div
			className={`h-full w-full ${className}`}
			data-testid="workflow-canvas"
		>
			<ReactFlow
				nodes={nodes}
				edges={edges}
				onNodesChange={onNodesChange}
				onEdgesChange={onEdgesChange}
				onConnect={onConnect}
				nodeTypes={nodeTypes}
				edgeTypes={edgeTypes}
				onInit={onInit}
				fitView
				snapToGrid
				snapGrid={[16, 16]}
				attributionPosition="bottom-right"
				style={{
					// Map agentd theme tokens to React Flow CSS variables so the
					// canvas inherits the active colour scheme automatically.
					// biome-ignore lint/suspicious/noExplicitAny: CSS custom property assignment
					["--xy-background-color" as any]: "var(--th-surface)",
					["--xy-node-border-color" as any]: "var(--th-border)",
					["--xy-edge-stroke" as any]: "var(--th-text-muted)",
					["--xy-edge-stroke-selected" as any]: "var(--th-accent)",
					["--xy-minimap-background-color" as any]: "var(--th-surface-sunken)",
					["--xy-controls-button-background-color" as any]: "var(--th-surface)",
					["--xy-controls-button-border-color" as any]: "var(--th-border)",
					["--xy-controls-button-color" as any]: "var(--th-text-secondary)",
					["--xy-controls-button-background-color-hover" as any]:
						"var(--th-surface-hover)",
				}}
			>
				<Controls />
				<MiniMap
					nodeColor={() => "var(--th-accent)"}
					maskColor="rgba(0,0,0,0.12)"
				/>
				<Background
					variant={BackgroundVariant.Dots}
					gap={16}
					size={1}
					color="var(--th-border)"
				/>
			</ReactFlow>
		</div>
	);
}

/**
 * WorkflowCanvas wraps the inner canvas in a ReactFlowProvider so it can be
 * dropped into any page without requiring a provider higher in the tree.
 * If you need multiple canvases on one page, render each inside its own
 * WorkflowCanvas (each gets its own provider context).
 */
export function WorkflowCanvas(props: WorkflowCanvasProps) {
	return (
		<ReactFlowProvider>
			<Canvas {...props} />
		</ReactFlowProvider>
	);
}

export default WorkflowCanvas;
