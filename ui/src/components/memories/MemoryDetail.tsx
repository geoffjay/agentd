/**
 * MemoryDetail — drawer content for a single memory record.
 *
 * Shows all memory details including full content, type, visibility,
 * tags, creator, references, and action buttons.
 */

import { Clock, Eye, Globe, Lock, Trash2, Users } from "lucide-react";
import type { Memory, VisibilityLevel } from "@/types/memory";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TYPE_STYLES: Record<string, string> = {
	information: "bg-th-status-info-bg text-th-status-info-text",
	question: "bg-th-status-warning-bg text-th-status-warning-text",
	request: "bg-th-status-info-bg text-th-status-info-text",
};

const TYPE_LABELS: Record<string, string> = {
	information: "Information",
	question: "Question",
	request: "Request",
};

const VISIBILITY_STYLES: Record<string, string> = {
	public: "bg-th-status-success-bg text-th-status-success-text",
	shared: "bg-th-status-warning-bg text-th-status-warning-text",
	private: "bg-th-status-error-bg text-th-status-error-text",
};

const VISIBILITY_ICONS: Record<VisibilityLevel, React.ReactNode> = {
	public: <Globe size={12} aria-hidden="true" />,
	shared: <Users size={12} aria-hidden="true" />,
	private: <Lock size={12} aria-hidden="true" />,
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MemoryDetailProps {
	memory: Memory;
	onEditVisibility: (memory: Memory) => void;
	onDelete: (id: string) => void;
}

export function MemoryDetail({
	memory,
	onEditVisibility,
	onDelete,
}: MemoryDetailProps) {
	return (
		<div className="space-y-5">
			{/* Badges */}
			<div className="flex flex-wrap items-center gap-2">
				<span
					className={[
						"inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium",
						TYPE_STYLES[memory.type] ?? "bg-th-surface-sunken text-th-text-muted",
					].join(" ")}
				>
					{TYPE_LABELS[memory.type] ?? memory.type}
				</span>
				<span
					className={[
						"inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium",
						VISIBILITY_STYLES[memory.visibility] ?? "bg-th-surface-sunken text-th-text-muted",
					].join(" ")}
				>
					{VISIBILITY_ICONS[memory.visibility]}
					{memory.visibility}
				</span>
			</div>

			{/* Content */}
			<div>
				<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
					Content
				</h3>
				<div className="mt-2 rounded-lg bg-th-surface-sunken p-4 text-sm text-th-text-secondary whitespace-pre-wrap leading-relaxed">
					{memory.content}
				</div>
			</div>

			{/* Tags */}
			{memory.tags.length > 0 && (
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Tags
					</h3>
					<div className="mt-2 flex flex-wrap gap-1.5">
						{memory.tags.map((tag) => (
							<span
								key={tag}
								className="rounded-full bg-th-surface-sunken px-2.5 py-0.5 text-xs font-medium text-th-text-secondary"
							>
								{tag}
							</span>
						))}
					</div>
				</div>
			)}

			{/* Creator & timestamps */}
			<div className="grid grid-cols-2 gap-4">
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Created By
					</h3>
					<p className="mt-1 text-sm text-th-text-secondary">
						{memory.created_by}
					</p>
				</div>
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Created
					</h3>
					<div className="mt-1 flex items-center gap-1.5 text-sm text-th-text-secondary">
						<Clock size={13} className="text-th-text-muted" />
						{new Date(memory.created_at).toLocaleString()}
					</div>
				</div>
			</div>

			{/* Shared with */}
			{memory.visibility === "shared" && memory.shared_with.length > 0 && (
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						Shared With
					</h3>
					<div className="mt-2 flex flex-wrap gap-1.5">
						{memory.shared_with.map((actor) => (
							<span
								key={actor}
								className="rounded-full bg-th-status-warning-bg px-2.5 py-0.5 text-xs font-medium text-th-status-warning-text"
							>
								{actor}
							</span>
						))}
					</div>
				</div>
			)}

			{/* References */}
			{memory.references.length > 0 && (
				<div>
					<h3 className="text-xs font-medium uppercase tracking-wide text-th-text-muted">
						References
					</h3>
					<ul className="mt-2 space-y-1">
						{memory.references.map((ref) => (
							<li
								key={ref}
								className="text-xs font-mono text-th-text-muted"
							>
								{ref}
							</li>
						))}
					</ul>
				</div>
			)}

			{/* Actions */}
			<div className="border-t border-th-border pt-4">
				<div className="flex gap-2">
					<button
						type="button"
						onClick={() => onEditVisibility(memory)}
						className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-th-surface-sunken text-th-text-secondary hover:bg-th-surface-hover transition-colors"
					>
						<Eye size={13} />
						Edit Visibility
					</button>
					<button
						type="button"
						onClick={() => onDelete(memory.id)}
						className="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium bg-th-status-error-bg text-th-status-error-text hover:opacity-90 transition-colors"
					>
						<Trash2 size={13} />
						Delete
					</button>
				</div>
			</div>
		</div>
	);
}

export default MemoryDetail;
