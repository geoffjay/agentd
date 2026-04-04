/**
 * MemorySearch — semantic similarity search panel for memories.
 *
 * Features:
 * - Search input with submit button (Enter key or click)
 * - Expandable advanced filters (type, tag, date range, limit)
 * - Results displayed as MemoryCard components
 * - Loading spinner during search
 * - Empty state when no results found
 * - Clear/reset button to return to default list view
 */

import { ChevronDown, ChevronRight, Loader2, Search, X } from "lucide-react";
import { useState } from "react";
import { MemoryCard } from "@/components/memories/MemoryCard";
import { useMemorySearch } from "@/hooks/useMemorySearch";
import type { Memory, MemoryType, SearchRequest } from "@/types/memory";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface MemorySearchProps {
	/** Called when the user wants to switch back to list view. */
	onSwitchToList: () => void;
	/** Called when user clicks edit visibility on a search result. */
	onEditVisibility: (memory: Memory) => void;
	/** Called when user clicks delete on a search result. */
	onDelete: (id: string) => void;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TYPE_OPTIONS: Array<{ value: MemoryType | ""; label: string }> = [
	{ value: "", label: "Any type" },
	{ value: "information", label: "Information" },
	{ value: "question", label: "Question" },
	{ value: "request", label: "Request" },
];

const LIMIT_OPTIONS = [5, 10, 20, 50];

const selectClass =
	"rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-sm text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring";

const inputClass =
	"rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-sm text-th-text-secondary placeholder-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function MemorySearch({
	onSwitchToList,
	onEditVisibility,
	onDelete,
}: MemorySearchProps) {
	const { results, total, searching, error, search, clear } = useMemorySearch();

	// Search form state
	const [query, setQuery] = useState("");
	const [showAdvanced, setShowAdvanced] = useState(false);
	const [filterType, setFilterType] = useState<MemoryType | "">("");
	const [filterTag, setFilterTag] = useState("");
	const [filterFrom, setFilterFrom] = useState("");
	const [filterTo, setFilterTo] = useState("");
	const [limit, setLimit] = useState(10);
	const [hasSearched, setHasSearched] = useState(false);

	const handleSearch = () => {
		if (!query.trim()) return;
		const request: SearchRequest = {
			query: query.trim(),
			limit,
			...(filterType ? { type: filterType } : {}),
			...(filterTag
				? {
						tags: filterTag
							.split(",")
							.map((t) => t.trim())
							.filter(Boolean),
					}
				: {}),
			...(filterFrom ? { from: filterFrom } : {}),
			...(filterTo ? { to: filterTo } : {}),
		};
		search(request);
		setHasSearched(true);
	};

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === "Enter") handleSearch();
	};

	const handleClear = () => {
		setQuery("");
		setFilterType("");
		setFilterTag("");
		setFilterFrom("");
		setFilterTo("");
		setLimit(10);
		setHasSearched(false);
		clear();
	};

	const handleBackToList = () => {
		handleClear();
		onSwitchToList();
	};

	return (
		<div>
			{/* Search input row */}
			<div className="flex items-center gap-2 mb-3">
				<div className="relative flex-1">
					<Search
						size={16}
						className="absolute left-3 top-1/2 -translate-y-1/2 text-th-text-muted"
						aria-hidden="true"
					/>
					<input
						type="text"
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						onKeyDown={handleKeyDown}
						placeholder="Semantic search across memories…"
						aria-label="Semantic search query"
						className="w-full rounded-md border border-th-border-input bg-th-input pl-9 pr-3 py-2 text-sm text-th-text-secondary placeholder-th-text-faint focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
					/>
				</div>

				<button
					type="button"
					onClick={handleSearch}
					disabled={!query.trim() || searching}
					className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover disabled:opacity-50 transition-colors"
				>
					{searching ? (
						<Loader2 size={16} className="animate-spin" />
					) : (
						"Search"
					)}
				</button>

				{hasSearched && (
					<button
						type="button"
						onClick={handleClear}
						aria-label="Clear search"
						className="rounded-md p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text transition-colors"
					>
						<X size={16} />
					</button>
				)}
			</div>

			{/* Advanced filters toggle */}
			<button
				type="button"
				onClick={() => setShowAdvanced((v) => !v)}
				aria-expanded={showAdvanced}
				className="mb-3 flex items-center gap-1 text-xs text-th-text-muted hover:text-th-text-secondary"
			>
				{showAdvanced ? (
					<ChevronDown size={12} aria-hidden="true" />
				) : (
					<ChevronRight size={12} aria-hidden="true" />
				)}
				Advanced filters
			</button>

			{/* Advanced filters panel */}
			{showAdvanced && (
				<div className="mb-4 flex flex-wrap items-center gap-2 rounded-lg border border-th-border-nav bg-th-surface-raised p-3">
					{/* Type filter */}
					<select
						aria-label="Filter by type"
						value={filterType}
						onChange={(e) => setFilterType(e.target.value as MemoryType | "")}
						className={selectClass}
					>
						{TYPE_OPTIONS.map((opt) => (
							<option key={opt.value} value={opt.value}>
								{opt.label}
							</option>
						))}
					</select>

					{/* Tag filter */}
					<input
						type="text"
						aria-label="Filter by tags"
						placeholder="Tags…"
						value={filterTag}
						onChange={(e) => setFilterTag(e.target.value)}
						className={[inputClass, "w-28"].join(" ")}
					/>

					{/* From date */}
					<input
						type="date"
						aria-label="From date"
						value={filterFrom}
						onChange={(e) => setFilterFrom(e.target.value)}
						className={[inputClass, "w-36"].join(" ")}
					/>

					{/* To date */}
					<input
						type="date"
						aria-label="To date"
						value={filterTo}
						onChange={(e) => setFilterTo(e.target.value)}
						className={[inputClass, "w-36"].join(" ")}
					/>

					{/* Limit */}
					<select
						aria-label="Result limit"
						value={limit}
						onChange={(e) => setLimit(Number(e.target.value))}
						className={selectClass}
					>
						{LIMIT_OPTIONS.map((n) => (
							<option key={n} value={n}>
								Max {n}
							</option>
						))}
					</select>
				</div>
			)}

			{/* Back to list link */}
			<div className="mb-4">
				<button
					type="button"
					onClick={handleBackToList}
					className="text-xs text-th-text-link hover:opacity-80 transition-colors"
				>
					← Back to memory list
				</button>
			</div>

			{/* Search loading */}
			{searching && (
				<div className="flex items-center justify-center gap-2 py-12 text-sm text-th-text-muted">
					<Loader2 size={16} className="animate-spin" />
					Searching memories…
				</div>
			)}

			{/* Search error */}
			{!searching && error && (
				<div className="rounded-lg border border-th-status-error-border bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text">
					{error}
				</div>
			)}

			{/* No results */}
			{!searching && !error && hasSearched && results.length === 0 && (
				<div className="py-12 text-center">
					<Search
						size={32}
						className="mx-auto mb-3 text-th-text-faint"
						aria-hidden="true"
					/>
					<p className="text-th-text-muted">No matching memories found</p>
					<p className="mt-1 text-xs text-th-text-faint">
						Try a different query or adjust the advanced filters.
					</p>
				</div>
			)}

			{/* Search results */}
			{!searching && results.length > 0 && (
				<>
					<div className="mb-3 text-xs text-th-text-muted">
						{total} result{total !== 1 ? "s" : ""} for &ldquo;{query}&rdquo;
					</div>
					<ul className="space-y-3" aria-label="Search results">
						{results.map((m) => (
							<li key={m.id}>
								<MemoryCard
									memory={m}
									onEditVisibility={onEditVisibility}
									onDelete={onDelete}
								/>
							</li>
						))}
					</ul>
				</>
			)}

			{/* Initial state (before any search) */}
			{!searching && !error && !hasSearched && (
				<div className="py-12 text-center">
					<Search
						size={32}
						className="mx-auto mb-3 text-th-text-faint"
						aria-hidden="true"
					/>
					<p className="text-th-text-muted">Enter a query to search memories</p>
					<p className="mt-1 text-xs text-th-text-faint">
						Uses semantic similarity to find the most relevant memories.
					</p>
				</div>
			)}
		</div>
	);
}

export default MemorySearch;
