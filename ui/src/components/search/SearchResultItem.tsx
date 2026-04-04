/**
 * SearchResultItem — a single row in the search palette results list.
 *
 * Shows: icon, title, subtitle, and a category badge.
 * Supports keyboard focus highlighting.
 */

import { Bell, Bot, Brain, ChevronRight, Zap } from "lucide-react";
import type { SearchResult } from "@/hooks/useSearch";

// ---------------------------------------------------------------------------
// Category icon + badge colour
// ---------------------------------------------------------------------------

const CATEGORY_META: Record<
	SearchResult["category"],
	{ label: string; iconEl: React.ReactNode; badgeClass: string }
> = {
	agent: {
		label: "Agent",
		iconEl: <Bot size={16} className="text-th-text-link" />,
		badgeClass: "bg-th-accent/10 text-th-text-link",
	},
	notification: {
		label: "Notification",
		iconEl: <Bell size={16} className="text-th-status-warning-text" />,
		badgeClass: "bg-th-status-warning-bg text-th-status-warning-text",
	},
	action: {
		label: "Action",
		iconEl: <Zap size={16} className="text-th-status-success-text" />,
		badgeClass: "bg-th-status-success-bg text-th-status-success-text",
	},
	memory: {
		label: "Memory",
		iconEl: <Brain size={16} className="text-th-status-info-text" />,
		badgeClass: "bg-th-status-info-bg text-th-status-info-text",
	},
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface SearchResultItemProps {
	result: SearchResult;
	isActive: boolean;
	onClick: (result: SearchResult) => void;
}

export function SearchResultItem({
	result,
	isActive,
	onClick,
}: SearchResultItemProps) {
	const meta = CATEGORY_META[result.category];

	function handleClick() {
		onClick(result);
	}

	function handleKeyDown(e: React.KeyboardEvent) {
		if (e.key === "Enter" || e.key === " ") {
			e.preventDefault();
			onClick(result);
		}
	}

	return (
		<li role="option" aria-selected={isActive}>
			<div
				role="button"
				tabIndex={-1}
				onClick={handleClick}
				onKeyDown={handleKeyDown}
				data-active={isActive}
				className={[
					"flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors",
					isActive
						? "bg-th-accent/10"
						: "hover:bg-th-surface-hover",
				].join(" ")}
			>
				{/* Category icon */}
				<span
					className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-th-surface-sunken"
					aria-hidden="true"
				>
					{meta.iconEl}
				</span>

				{/* Text */}
				<span className="min-w-0 flex-1">
					<span className="block truncate text-sm font-medium text-th-text">
						{result.title}
					</span>
					<span className="block truncate text-xs text-th-text-muted">
						{result.subtitle}
					</span>
				</span>

				{/* Category badge */}
				<span
					className={[
						"shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium",
						meta.badgeClass,
					].join(" ")}
				>
					{meta.label}
				</span>

				{/* Arrow hint */}
				<ChevronRight
					size={14}
					className="shrink-0 text-th-text-faint"
					aria-hidden="true"
				/>
			</div>
		</li>
	);
}

export default SearchResultItem;
