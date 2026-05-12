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
 *
 * When `readOnly` is true the canvas disables drag, connect, and delete
 * interactions while keeping zoom and pan fully functional.
 */

import "@xyflow/react/dist/style.css";
import {
	Background,
	BackgroundVariant,
	Controls,
	MiniMap,
	ReactFlow,
	ReactFlowProvider,
	useReactFlow,
	type Edge,
	type EdgeTypes,
	type Node,
	type NodeTypes,
	type OnConnect,
	type OnEdgesChange,
	type OnNodesChange,
	type ReactFlowInstance,
} from "@xyflow/react";
import { useEffect } from "react";

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
	/**
	 * When true, disables drag, connect, and delete interactions.
	 * Zoom and pan remain active.  A "View mode" badge is shown.
	 */
	readOnly?: boolean;
	/**
	 * Called once after React Flow initialises, exposing the instance
	 * so callers can use helpers such as screenToFlowPosition.
	 */
	onInit?: (instance: ReactFlowInstance) => void;
}

// ---------------------------------------------------------------------------
// Minimap node colour helper
// ---------------------------------------------------------------------------

function minimapNodeColor(node: Node): string {
	switch (node.type) {
		case "trigger":
			return "var(--th-accent)";
		case "agent":
			return "#22c55e"; // green-500 — matches AgentNode running border
		default:
			return "var(--th-text-muted)";
	}
}

// ---------------------------------------------------------------------------
// FitViewOnLoad — triggers fitView after layout data is available
// ---------------------------------------------------------------------------

function FitViewOnLoad({ readOnly }: { readOnly?: boolean }) {
	const { fitView } = useReactFlow();
	useEffect(() => {
		// Small delay lets React Flow measure node sizes before fitting
		const id = window.setTimeout(() => fitView({ padding: 0.2 }), 50);
		return () => window.clearTimeout(id);
		// We only want this to run once on mount
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [readOnly]);
	return null;
}

// ---------------------------------------------------------------------------
// Inner canvas (must live inside a ReactFlowProvider)
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
	readOnly = false,
	onInit,
}: WorkflowCanvasProps) {
	return (
		<div
			className={`h-full w-full relative ${className}`}
			data-testid="workflow-canvas"
			data-readonly={readOnly}
		>
			{readOnly && (
				<div
					className="absolute top-2 left-2 z-10 flex items-center gap-1.5 rounded-full border border-th-border bg-th-surface px-2.5 py-1 text-xs font-medium text-th-text-muted shadow-sm pointer-events-none"
					data-testid="readonly-badge"
				>
					<span className="h-1.5 w-1.5 rounded-full bg-th-text-muted" />
					View mode
				</div>
			)}

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
				snapToGrid={!readOnly}
				snapGrid={[16, 16]}
				attributionPosition="bottom-right"
				// Read-only interaction flags
				nodesDraggable={!readOnly}
				nodesConnectable={!readOnly}
				deleteKeyCode={readOnly ? null : "Delete"}
				// Always allow viewport navigation
				panOnDrag
				zoomOnScroll
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
					nodeColor={minimapNodeColor}
					maskColor="rgba(0,0,0,0.12)"
				/>
				<Background
					variant={BackgroundVariant.Dots}
					gap={16}
					size={1}
					color="var(--th-border)"
				/>
				<FitViewOnLoad readOnly={readOnly} />
			</ReactFlow>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Public component
// ---------------------------------------------------------------------------

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
