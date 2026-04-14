/**
 * WorkflowBuilder — visual workflow composition page.
 *
 * Layout:
 *   ┌─────────────────────────────────────────────────┐
 *   │  Header: name input  │  [Save]  [Cancel]        │
 *   ├──────────┬──────────────────────────────────────┤
 *   │ NodePal  │  WorkflowCanvas (React Flow)          │
 *   │  ette    │                                       │
 *   ├──────────┴──────────────────────────────────────┤
 *   │  Status bar: validation errors / save state      │
 *   └─────────────────────────────────────────────────┘
 *
 * Routes:
 *   /workflows/builder         — create new workflow(s)
 *   /workflows/:id/edit        — edit an existing workflow
 */

import {
	AlertCircle,
	CheckCircle,
	Loader2,
	Save,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
	useEdgesState,
	useNodesState,
	type Connection,
	type Edge,
	type Node,
} from "@xyflow/react";
import { WorkflowCanvas } from "@/components/workflows/canvas/WorkflowCanvas";
import {
	NodePalette,
	PALETTE_DRAG_KEY,
	decodeDragData,
} from "@/components/workflows/canvas/NodePalette";
import { workflowNodeTypes, workflowEdgeTypes } from "@/components/workflows/canvas/nodeTypes";
import type { AgentNodeData } from "@/components/workflows/canvas/nodes/AgentNode";
import type { TriggerNodeData } from "@/components/workflows/canvas/nodes/TriggerNode";
import type { PromptEdgeData } from "@/components/workflows/canvas/edges/PromptEdge";
import {
	graphToWorkflows,
	validateGraph,
	workflowsToGraph,
	loadLayout,
	saveLayout,
	layoutStorageKey,
} from "@/components/workflows/canvas/serialization";
import type { SerializationError } from "@/components/workflows/canvas/serialization";
import { useAgents } from "@/hooks/useAgents";
import { orchestratorClient } from "@/services/orchestrator";
import type { TriggerType } from "@/types/orchestrator";
import {
	getTriggerCategory,
	getDefaultTriggerConfig,
} from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function newNodeId(): string {
	return `node-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

function newEdgeId(): string {
	return `edge-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function WorkflowBuilder() {
	const navigate = useNavigate();
	const { id: editWorkflowId } = useParams<{ id?: string }>();
	const isEditing = Boolean(editWorkflowId);

	// React Flow state
	const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
	const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);

	// Builder state
	const [workflowName, setWorkflowName] = useState("");
	const [isDirty, setIsDirty] = useState(false);
	const [saving, setSaving] = useState(false);
	const [saveError, setSaveError] = useState<string | undefined>();
	const [saveSuccess, setSaveSuccess] = useState(false);
	const [validationErrors, setValidationErrors] = useState<SerializationError[]>([]);
	const [loading, setLoading] = useState(isEditing);

	// React Flow instance ref for screenToFlowPosition
	const rfInstanceRef = useRef<{ screenToFlowPosition: (pos: { x: number; y: number }) => { x: number; y: number } } | null>(null);

	const { allAgents } = useAgents({ pageSize: 200 });

	// ── Load existing workflow ─────────────────────────────────────────────
	useEffect(() => {
		if (!editWorkflowId) return;

		setLoading(true);
		orchestratorClient
			.getWorkflow(editWorkflowId)
			.then((wf) => {
				setWorkflowName(wf.name);
				const layout = loadLayout([wf.id]);
				const { nodes: n, edges: e } = workflowsToGraph([wf], allAgents, layout ?? undefined);
				setNodes(n);
				setEdges(e);
			})
			.catch((err) => {
				setSaveError(
					err instanceof Error ? err.message : "Failed to load workflow",
				);
			})
			.finally(() => setLoading(false));
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [editWorkflowId]);

	// ── Dirty tracking ─────────────────────────────────────────────────────
	const markDirty = useCallback(() => {
		setIsDirty(true);
		setSaveSuccess(false);
	}, []);

	// Intercept node/edge changes to mark dirty
	const handleNodesChange = useCallback(
		(changes: Parameters<typeof onNodesChange>[0]) => {
			onNodesChange(changes);
			markDirty();
		},
		[onNodesChange, markDirty],
	);

	const handleEdgesChange = useCallback(
		(changes: Parameters<typeof onEdgesChange>[0]) => {
			onEdgesChange(changes);
			markDirty();
		},
		[onEdgesChange, markDirty],
	);

	// ── Browser unload guard ───────────────────────────────────────────────
	useEffect(() => {
		if (!isDirty) return;
		function handleBeforeUnload(e: BeforeUnloadEvent) {
			e.preventDefault();
		}
		window.addEventListener("beforeunload", handleBeforeUnload);
		return () => window.removeEventListener("beforeunload", handleBeforeUnload);
	}, [isDirty]);

	// ── Connection validation ──────────────────────────────────────────────
	const handleConnect = useCallback(
		(connection: Connection) => {
			const srcNode = nodes.find((n) => n.id === connection.source);
			const tgtNode = nodes.find((n) => n.id === connection.target);

			// Only allow trigger → agent
			if (srcNode?.type !== "trigger" || tgtNode?.type !== "agent") return;

			// Prevent duplicate edges for same source/target pair
			const duplicate = edges.some(
				(e) =>
					e.source === connection.source &&
					e.target === connection.target,
			);
			if (duplicate) return;

			const newEdge: Edge<PromptEdgeData> = {
				id: newEdgeId(),
				source: connection.source!,
				target: connection.target!,
				type: "prompt",
				data: {
					promptTemplate: "",
					pollIntervalSecs: 300,
					enabled: true,
				},
			};
			setEdges((prev) => [...prev, newEdge]);
			markDirty();
		},
		[nodes, edges, setEdges, markDirty],
	);

	// ── Drag-and-drop from palette ─────────────────────────────────────────
	function handleDragOver(e: React.DragEvent) {
		e.preventDefault();
		e.dataTransfer.dropEffect = "copy";
	}

	function handleDrop(e: React.DragEvent) {
		e.preventDefault();
		const raw = e.dataTransfer.getData(PALETTE_DRAG_KEY);
		if (!raw) return;

		const dragData = decodeDragData(raw);
		if (!dragData) return;

		// Convert screen → flow coordinates
		const canvasEl = e.currentTarget as HTMLElement;
		const rect = canvasEl.getBoundingClientRect();
		const screenPos = { x: e.clientX - rect.left, y: e.clientY - rect.top };
		const position = rfInstanceRef.current
			? rfInstanceRef.current.screenToFlowPosition(screenPos)
			: screenPos;

		if (dragData.type === "trigger") {
			const triggerType = dragData.triggerType as TriggerType;
			const triggerConfig = getDefaultTriggerConfig(triggerType);
			const category = getTriggerCategory(triggerType);

			const newNode: Node<TriggerNodeData> = {
				id: newNodeId(),
				type: "trigger",
				position,
				data: {
					triggerConfig,
					category,
					enabled: true,
				},
			};
			setNodes((prev) => [...prev, newNode]);
		} else if (dragData.type === "agent") {
			const agent = allAgents.find((a) => a.id === dragData.agentId);
			if (!agent) return;

			const newNode: Node<AgentNodeData> = {
				id: newNodeId(),
				type: "agent",
				position,
				data: {
					agentId: agent.id,
					name: agent.name,
					status: agent.status,
					model: agent.config?.model,
					toolPolicy: agent.config.tool_policy,
				},
			};
			setNodes((prev) => [...prev, newNode]);
		}

		markDirty();
	}

	// ── Save ───────────────────────────────────────────────────────────────
	async function handleSave() {
		const errors = validateGraph(nodes, edges);
		setValidationErrors(errors);
		if (errors.length > 0) return;

		setSaving(true);
		setSaveError(undefined);
		setSaveSuccess(false);

		try {
			const requests = graphToWorkflows(nodes, edges);
			const saved = await Promise.all(
				requests.map((req) => {
					const named = workflowName.trim()
						? { ...req, name: workflowName.trim() }
						: req;
					return orchestratorClient.createWorkflow(named);
				}),
			);

			// Persist layout
			const ids = saved.map((w) => w.id);
			saveLayout(ids, {
				nodes: Object.fromEntries(
					nodes.map((n) => [n.id, n.position]),
				),
				viewport: { x: 0, y: 0, zoom: 1 },
			});

			setIsDirty(false);
			setSaveSuccess(true);

			// Navigate to the first saved workflow's detail page after a beat
			if (saved.length === 1) {
				setTimeout(() => navigate(`/workflows/${saved[0].id}`), 800);
			} else {
				setTimeout(() => navigate("/workflows"), 800);
			}
		} catch (err) {
			setSaveError(
				err instanceof Error ? err.message : "Failed to save workflow",
			);
		} finally {
			setSaving(false);
		}
	}

	function handleCancel() {
		navigate(isEditing ? `/workflows/${editWorkflowId}` : "/workflows");
	}

	// ── Render ─────────────────────────────────────────────────────────────
	const hasErrors = validationErrors.length > 0;

	return (
		<div
			className="flex flex-col h-full"
			data-testid="workflow-builder"
		>
			{/* ── Header ───────────────────────────────────────────────── */}
			<div className="flex items-center gap-3 border-b border-th-border bg-th-surface px-4 py-2 flex-shrink-0">
				<input
					type="text"
					value={workflowName}
					onChange={(e) => {
						setWorkflowName(e.target.value);
						markDirty();
					}}
					placeholder="Workflow name…"
					data-testid="builder-name-input"
					className="flex-1 rounded border border-th-border-input bg-th-input px-3 py-1.5 text-sm text-th-text placeholder:text-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
				/>

				<button
					type="button"
					onClick={handleSave}
					disabled={saving || loading}
					data-testid="builder-save-btn"
					className="flex items-center gap-1.5 rounded bg-th-accent px-3 py-1.5 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
				>
					{saving ? (
						<Loader2 size={14} className="animate-spin" />
					) : (
						<Save size={14} />
					)}
					{saving ? "Saving…" : "Save"}
				</button>

				<button
					type="button"
					onClick={handleCancel}
					disabled={saving}
					data-testid="builder-cancel-btn"
					className="flex items-center gap-1.5 rounded border border-th-border-strong px-3 py-1.5 text-sm text-th-text-secondary hover:bg-th-surface-hover disabled:opacity-50 transition-colors"
				>
					<X size={14} />
					Cancel
				</button>

				{isDirty && !saving && !saveSuccess && (
					<span className="text-xs text-th-text-muted" data-testid="dirty-indicator">
						Unsaved changes
					</span>
				)}
			</div>

			{/* ── Main area: palette + canvas ───────────────────────────── */}
			<div className="flex flex-1 overflow-hidden">
				<NodePalette agents={allAgents} />

				<div
					className="flex-1 relative"
					onDragOver={handleDragOver}
					onDrop={handleDrop}
					data-testid="builder-canvas-area"
				>
					{loading ? (
						<div className="flex items-center justify-center h-full">
							<Loader2 size={24} className="animate-spin text-th-text-muted" />
						</div>
					) : (
						<WorkflowCanvas
							nodes={nodes}
							edges={edges}
							onNodesChange={handleNodesChange}
							onEdgesChange={handleEdgesChange}
							onConnect={handleConnect}
							nodeTypes={workflowNodeTypes}
							edgeTypes={workflowEdgeTypes}
							className="h-full"
						/>
					)}
				</div>
			</div>

			{/* ── Status bar ───────────────────────────────────────────── */}
			{(hasErrors || saveError || saveSuccess) && (
				<div
					className={[
						"flex items-center gap-2 border-t px-4 py-2 text-xs flex-shrink-0",
						saveSuccess
							? "border-green-300 bg-green-50 dark:bg-green-950 text-green-700 dark:text-green-300"
							: "border-th-status-error-border bg-th-status-error-bg text-th-status-error-text",
					].join(" ")}
					data-testid="builder-status-bar"
				>
					{saveSuccess ? (
						<>
							<CheckCircle size={13} />
							<span>Saved — redirecting…</span>
						</>
					) : (
						<>
							<AlertCircle size={13} />
							<span>
								{saveError ??
									validationErrors.map((e) => e.message).join(" · ")}
							</span>
						</>
					)}
				</div>
			)}
		</div>
	);
}

export default WorkflowBuilder;
