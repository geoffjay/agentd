/**
 * SearchPalette — full-screen command palette overlay.
 *
 * Features:
 * - Opens via Ctrl+K / Cmd+K or by clicking the search button in the header
 * - Debounced search-as-you-type across agents and notifications
 * - Keyboard navigation: ↑/↓ to move, Enter to navigate, Escape to close
 * - Recent searches shown when input is empty
 * - Backdrop click to close
 * - Portal to document.body to avoid z-index issues
 */

import { Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import type { SearchResult } from "@/hooks/useSearch";
import { useSearch } from "@/hooks/useSearch";
import { RecentSearches, SearchResults } from "./SearchResults";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Flatten all result groups into an ordered list for keyboard navigation */
function flattenResults(
	results: ReturnType<typeof useSearch>["results"],
): SearchResult[] {
	return [
		...results.actions,
		...results.agents,
		...results.notifications,
		...results.memories,
	];
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface SearchPaletteProps {
	isOpen: boolean;
	onClose: () => void;
}

export function SearchPalette({ isOpen, onClose }: SearchPaletteProps) {
	const navigate = useNavigate();
	const {
		query,
		setQuery,
		results,
		loading,
		recentSearches,
		addRecentSearch,
		clearRecentSearches,
	} = useSearch();
	const inputRef = useRef<HTMLInputElement>(null);
	const [activeIndex, setActiveIndex] = useState(-1);

	const allResults = flattenResults(results);
	const activeId =
		activeIndex >= 0 && activeIndex < allResults.length
			? allResults[activeIndex].id
			: null;

	// Focus input when palette opens; reset state when it closes
	useEffect(() => {
		if (isOpen) {
			setActiveIndex(-1);
			setTimeout(() => inputRef.current?.focus(), 0);
		} else {
			setQuery("");
			setActiveIndex(-1);
		}
	}, [isOpen, setQuery]);

	// Navigate to a result
	const selectResult = useCallback(
		(result: SearchResult) => {
			if (query.trim()) addRecentSearch(query.trim());
			onClose();
			navigate(result.href);
		},
		[query, addRecentSearch, onClose, navigate],
	);

	// Keyboard navigation within palette
	const handleKeyDown = useCallback(
		(e: React.KeyboardEvent) => {
			switch (e.key) {
				case "ArrowDown": {
					e.preventDefault();
					setActiveIndex((i) => Math.min(i + 1, allResults.length - 1));
					break;
				}
				case "ArrowUp": {
					e.preventDefault();
					setActiveIndex((i) => Math.max(i - 1, -1));
					break;
				}
				case "Enter": {
					e.preventDefault();
					if (activeIndex >= 0 && activeIndex < allResults.length) {
						selectResult(allResults[activeIndex]);
					}
					break;
				}
				case "Escape": {
					e.preventDefault();
					onClose();
					break;
				}
				default:
					break;
			}
		},
		[activeIndex, allResults, selectResult, onClose],
	);

	// Reset active index when results change
	useEffect(() => {
		setActiveIndex(-1);
	}, [results]);

	if (!isOpen) return null;

	const showEmpty = !query.trim();

	return createPortal(
		<div
			role="dialog"
			aria-modal="true"
			aria-label="Global search"
			className="fixed inset-0 z-[100] flex items-start justify-center pt-[10vh] px-4"
		>
			{/* Backdrop */}
			<div
				aria-hidden="true"
				onClick={onClose}
				className="absolute inset-0 bg-th-overlay backdrop-blur-sm"
			/>

			{/* Palette panel */}
			<div
				className="relative z-10 overflow-hidden rounded-xl border border-th-border bg-th-surface shadow-2xl"
				onKeyDown={handleKeyDown}
			>
				{/* Search input row */}
				<div className="flex items-center gap-3 border-b border-th-border px-4 py-3">
					<Search
						size={18}
						className="shrink-0 text-th-text-faint"
						aria-hidden="true"
					/>
					<input
						ref={inputRef}
						type="search"
						role="combobox"
						aria-autocomplete="list"
						aria-expanded={!showEmpty}
						aria-controls="search-results"
						aria-activedescendant={activeId ?? undefined}
						placeholder="Search agents, notifications, pages…"
						aria-label="Search"
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						className="min-w-0 flex-1 bg-transparent text-sm text-th-text placeholder-th-text-faint outline-none"
					/>
					{query && (
						<button
							type="button"
							aria-label="Clear search"
							onClick={() => setQuery("")}
							className="shrink-0 rounded p-0.5 text-th-text-faint hover:text-th-text-secondary"
						>
							<X size={14} />
						</button>
					)}
					<kbd className="hidden shrink-0 rounded border border-th-border px-1.5 py-0.5 text-[10px] text-th-text-faint sm:block">
						Esc
					</kbd>
				</div>

				{/* Results area */}
				<div id="search-results" className="max-h-[60vh] overflow-y-auto">
					{showEmpty ? (
						<RecentSearches
							searches={recentSearches}
							onSelect={(q) => setQuery(q)}
							onClear={clearRecentSearches}
						/>
					) : (
						<SearchResults
							query={query}
							results={results}
							loading={loading}
							activeId={activeId}
							onSelect={selectResult}
						/>
					)}
				</div>

				{/* Footer hint */}
				<div className="flex items-center gap-4 border-t border-th-border px-4 py-2">
					<span className="flex items-center gap-1 text-[11px] text-th-text-faint">
						<kbd className="rounded border border-th-border px-1 py-0.5 text-[10px]">
							↑↓
						</kbd>
						navigate
					</span>
					<span className="flex items-center gap-1 text-[11px] text-th-text-faint">
						<kbd className="rounded border border-th-border px-1 py-0.5 text-[10px]">
							↵
						</kbd>
						open
					</span>
					<span className="flex items-center gap-1 text-[11px] text-th-text-faint">
						<kbd className="rounded border border-th-border px-1 py-0.5 text-[10px]">
							Esc
						</kbd>
						close
					</span>
				</div>
			</div>
		</div>,
		document.body,
	);
}

export default SearchPalette;
