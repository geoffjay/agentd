/**
 * MemoryCard — displays a single memory record with:
 * - Content preview with expand/collapse
 * - MemoryType badge with colour coding
 * - VisibilityLevel badge
 * - Tags displayed as chips
 * - Created by / created at metadata
 * - Action buttons: Edit visibility, Delete (with confirmation)
 */

import {
	ChevronDown,
	ChevronRight,
	Clock,
	Eye,
	Globe,
	Lock,
	Trash2,
	Users,
} from "lucide-react";
import { useState } from "react";
import type { Memory, VisibilityLevel } from "@/types/memory";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Max characters to show before truncating. */
const CONTENT_PREVIEW_LENGTH = 180;

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
	public: <Globe size={10} aria-hidden="true" />,
	shared: <Users size={10} aria-hidden="true" />,
	private: <Lock size={10} aria-hidden="true" />,
};

function formatRelativeTime(dateStr: string): string {
	const diffMs = Date.now() - new Date(dateStr).getTime();
	const diffSec = Math.floor(diffMs / 1000);
	if (diffSec < 60) return "just now";
	const diffMin = Math.floor(diffSec / 60);
	if (diffMin < 60) return `${diffMin}m ago`;
	const diffHour = Math.floor(diffMin / 60);
	if (diffHour < 24) return `${diffHour}h ago`;
	const diffDay = Math.floor(diffHour / 24);
	return `${diffDay}d ago`;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface MemoryCardProps {
	memory: Memory;
	onEditVisibility: (memory: Memory) => void;
	onDelete: (id: string) => void;
}

export function MemoryCard({
	memory,
	onEditVisibility,
	onDelete,
}: MemoryCardProps) {
	const [expanded, setExpanded] = useState(false);

	const isLong = memory.content.length > CONTENT_PREVIEW_LENGTH;
	const displayContent =
		expanded || !isLong
			? memory.content
			: memory.content.slice(0, CONTENT_PREVIEW_LENGTH) + "…";

	return (
		<article
			aria-label={`Memory: ${memory.content.slice(0, 60)}`}
			className="rounded-lg border border-th-border bg-th-surface transition-colors hover:border-th-border-strong"
		>
			<div className="p-4">
				{/* Top row: badges */}
				<div className="flex flex-wrap items-start gap-2">
					{/* Type badge */}
					<span
						className={[
							"shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
							TYPE_STYLES[memory.type] ?? "bg-th-surface-raised text-th-text-secondary",
						].join(" ")}
					>
						{TYPE_LABELS[memory.type] ?? memory.type}
					</span>

					{/* Visibility badge */}
					<span
						className={[
							"shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
							VISIBILITY_STYLES[memory.visibility] ??
								"bg-th-surface-raised text-th-text-secondary",
						].join(" ")}
					>
						{VISIBILITY_ICONS[memory.visibility]}
						{memory.visibility}
					</span>

					<div className="flex-1" />

					{/* Timestamp */}
					<span className="flex items-center gap-1 text-xs text-th-text-muted shrink-0">
						<Clock size={11} aria-hidden="true" />
						{formatRelativeTime(memory.created_at)}
					</span>
				</div>

				{/* Content */}
				<div className="mt-2">
					<p className="text-sm text-th-text-secondary whitespace-pre-wrap leading-relaxed">
						{displayContent}
					</p>

					{/* Expand/collapse toggle */}
					{isLong && (
						<button
							type="button"
							aria-expanded={expanded}
							onClick={() => setExpanded((v) => !v)}
							className="mt-1 flex items-center gap-1 text-xs text-th-text-muted hover:text-th-text-secondary"
						>
							{expanded ? (
								<ChevronDown size={12} aria-hidden="true" />
							) : (
								<ChevronRight size={12} aria-hidden="true" />
							)}
							{expanded ? "Show less" : "Show more"}
						</button>
					)}
				</div>

				{/* Tags */}
				{memory.tags.length > 0 && (
					<div className="mt-2 flex flex-wrap gap-1.5">
						{memory.tags.map((tag) => (
							<span
								key={tag}
								className="rounded-full bg-th-surface-raised px-2 py-0.5 text-[11px] font-medium text-th-text-secondary"
							>
								{tag}
							</span>
						))}
					</div>
				)}

				{/* Shared with */}
				{memory.visibility === "shared" && memory.shared_with.length > 0 && (
					<div className="mt-2 text-xs text-th-text-muted">
						Shared with: {memory.shared_with.join(", ")}
					</div>
				)}

				{/* Meta row + actions */}
				<div className="mt-3 flex flex-wrap items-center gap-3">
					{/* Creator */}
					<span className="text-xs text-th-text-muted">
						by <span className="text-th-text-muted">{memory.created_by}</span>
					</span>

					{/* References count */}
					{memory.references.length > 0 && (
						<span className="text-xs text-th-text-muted">
							{memory.references.length} ref
							{memory.references.length !== 1 ? "s" : ""}
						</span>
					)}

					<div className="flex-1" />

					{/* Action buttons */}
					<div className="flex gap-2">
						<button
							type="button"
							onClick={() => onEditVisibility(memory)}
							aria-label="Edit visibility"
							className="rounded px-2.5 py-1 text-xs font-medium bg-th-surface-raised text-th-text-secondary hover:bg-th-surface-hover transition-colors flex items-center gap-1"
						>
							<Eye size={12} aria-hidden="true" />
							Visibility
						</button>

						<button
							type="button"
							onClick={() => onDelete(memory.id)}
							aria-label="Delete memory"
							className="rounded px-2.5 py-1 text-xs font-medium bg-th-status-error-bg text-th-status-error-text hover:opacity-90 transition-colors flex items-center gap-1"
						>
							<Trash2 size={12} aria-hidden="true" />
							Delete
						</button>
					</div>
				</div>
			</div>
		</article>
	);
}

export default MemoryCard;
