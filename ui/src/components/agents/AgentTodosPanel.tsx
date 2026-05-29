/**
 * AgentTodosPanel — displays the current todo list maintained by the agent
 * via TodoWrite tool calls, updated in real-time from the WebSocket stream.
 *
 * Listens for `agent:tool_use` events where `tool_name === "TodoWrite"` and
 * replaces the displayed list each time the agent updates its todos.
 */

import { CheckSquare, Circle, Clock, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { agentEventBus } from "@/services/eventBus";
import type { AgentToolUseEvent } from "@/types/orchestrator";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type TodoStatus = "pending" | "in_progress" | "completed";
export type TodoPriority = "low" | "medium" | "high";

export interface TodoItem {
	id: string;
	content: string;
	status: TodoStatus;
	priority: TodoPriority;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Extract a TodoItem array from a TodoWrite tool_input value, or return null. */
function parseTodos(toolInput: Record<string, unknown>): TodoItem[] | null {
	const raw = toolInput["todos"];
	if (!Array.isArray(raw)) return null;
	return raw
		.filter(
			(item): item is Record<string, unknown> =>
				typeof item === "object" && item !== null,
		)
		.map((item) => ({
			id: String(item["id"] ?? ""),
			content: String(item["content"] ?? ""),
			status: (["pending", "in_progress", "completed"].includes(
				item["status"] as string,
			)
				? item["status"]
				: "pending") as TodoStatus,
			priority: (["low", "medium", "high"].includes(item["priority"] as string)
				? item["priority"]
				: "medium") as TodoPriority,
		}));
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

const STATUS_ICON: Record<TodoStatus, React.ReactNode> = {
	pending: (
		<Circle
			size={14}
			className="shrink-0 text-th-text-faint"
			aria-hidden="true"
		/>
	),
	in_progress: (
		<Loader2
			size={14}
			className="shrink-0 animate-spin text-th-status-info-text"
			aria-hidden="true"
		/>
	),
	completed: (
		<CheckSquare
			size={14}
			className="shrink-0 text-th-status-success-text"
			aria-hidden="true"
		/>
	),
};

const STATUS_LABEL: Record<TodoStatus, string> = {
	pending: "Pending",
	in_progress: "In progress",
	completed: "Completed",
};

const PRIORITY_BADGE: Record<TodoPriority, string> = {
	high: "bg-th-status-error-bg text-th-status-error-text",
	medium: "bg-th-status-warning-bg text-th-status-warning-text",
	low: "bg-th-surface-sunken text-th-text-muted",
};

interface TodoRowProps {
	item: TodoItem;
}

function TodoRow({ item }: TodoRowProps) {
	const isCompleted = item.status === "completed";
	return (
		<li
			className="flex items-start gap-2.5 py-2"
			aria-label={`${item.content} — ${STATUS_LABEL[item.status]}, ${item.priority} priority`}
		>
			<span className="mt-0.5">{STATUS_ICON[item.status]}</span>
			<span
				className={`flex-1 text-sm leading-snug ${
					isCompleted
						? "text-th-text-faint line-through"
						: "text-th-text-secondary"
				}`}
			>
				{item.content}
			</span>
			{item.priority !== "medium" && (
				<span
					className={`mt-0.5 shrink-0 rounded px-1.5 py-0.5 text-xs font-medium ${PRIORITY_BADGE[item.priority]}`}
				>
					{item.priority}
				</span>
			)}
		</li>
	);
}

// ---------------------------------------------------------------------------
// AgentTodosPanel
// ---------------------------------------------------------------------------

export interface AgentTodosPanelProps {
	agentId: string;
}

export function AgentTodosPanel({ agentId }: AgentTodosPanelProps) {
	const [todos, setTodos] = useState<TodoItem[] | null>(null);

	useEffect(() => {
		const unsubscribe = agentEventBus.on<AgentToolUseEvent>(
			"agent:tool_use",
			(event) => {
				if (event.agentId !== agentId) return;
				if (event.tool_name !== "TodoWrite") return;
				const parsed = parseTodos(event.tool_input);
				if (parsed !== null) {
					setTodos(parsed);
				}
			},
		);
		return unsubscribe;
	}, [agentId]);

	const renderEmpty = () => (
		<section
			aria-label="Agent todos"
			className="rounded-lg border border-th-border bg-th-surface"
		>
			<div className="flex items-center gap-2 border-b border-th-border px-4 py-3">
				<Clock size={16} aria-hidden="true" className="text-th-text-muted" />
				<h2 className="text-sm font-medium text-th-text">Todos</h2>
				<p className="py-2 text-sm text-th-text-faint">No todos.</p>
			</div>
		</section>
	);

	// Render an empty state until the agent has written at least one todo list
	if (todos === null) return renderEmpty();

	const pending = todos.filter((t) => t.status === "pending").length;
	const inProgress = todos.filter((t) => t.status === "in_progress").length;
	const completed = todos.filter((t) => t.status === "completed").length;

	return (
		<section
			aria-label="Agent todos"
			className="rounded-lg border border-th-border bg-th-surface"
		>
			<div className="flex items-center gap-2 border-b border-th-border px-4 py-3">
				<Clock size={16} aria-hidden="true" className="text-th-text-muted" />
				<h2 className="text-sm font-medium text-th-text">Todos</h2>
				{todos.length > 0 && (
					<span className="ml-auto flex items-center gap-1.5">
						{inProgress > 0 && (
							<span
								title={`${inProgress} in progress`}
								className="rounded-full bg-th-status-info-bg px-2 py-0.5 text-xs font-medium text-th-status-info-text"
							>
								{inProgress} active
							</span>
						)}
						<span
							title={`${completed} of ${todos.length} completed`}
							className="rounded-full bg-th-surface-sunken px-2 py-0.5 text-xs font-medium text-th-text-muted"
						>
							{completed}/{todos.length}
						</span>
					</span>
				)}
			</div>

			{/* Body */}
			<div className="px-4 py-2">
				{todos.length === 0 ? (
					<p className="py-2 text-sm text-th-text-faint">No todos.</p>
				) : (
					<ul
						className="divide-y divide-th-border"
						aria-label={`${todos.length} todo item${todos.length !== 1 ? "s" : ""}, ${pending} pending, ${inProgress} in progress, ${completed} completed`}
					>
						{todos.map((item) => (
							<TodoRow key={item.id} item={item} />
						))}
					</ul>
				)}
			</div>
		</section>
	);
}

export default AgentTodosPanel;
