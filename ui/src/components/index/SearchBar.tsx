/**
 * SearchBar — code search input with search mode selector (Vector / Keyword /
 * Hybrid), repository filter, language filter, file_pattern glob, and
 * hierarchy_level selector.
 *
 * Accepts optional initial-value props so the parent can drive state from URL
 * query params. Fires onSearch with a CodeSearchRequest when the user submits.
 * Shows a loading spinner while searchLoading is true.
 */

import { Search, X } from "lucide-react";
import { useState } from "react";
import type { CodeSearchMode, CodeSearchRequest, RepoRecord } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SEARCH_MODES: Array<{ value: CodeSearchMode; label: string }> = [
	{ value: "Hybrid", label: "Hybrid" },
	{ value: "Vector", label: "Vector" },
	{ value: "Keyword", label: "Keyword" },
];

const HIERARCHY_LEVELS: Array<{ value: string; label: string }> = [
	{ value: "", label: "Any level" },
	{ value: "symbol", label: "Symbol" },
	{ value: "file", label: "File" },
	{ value: "directory", label: "Directory" },
	{ value: "repository", label: "Repository" },
];

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface SearchBarProps {
	repositories: RepoRecord[];
	searchLoading: boolean;
	onSearch: (req: CodeSearchRequest) => Promise<void>;
	onClear: () => void;
	hasResults: boolean;
	/** Initial values driven from URL params or parent state. */
	initialQuery?: string;
	initialMode?: CodeSearchMode;
	initialRepoId?: string;
	initialLanguage?: string;
	initialFilePattern?: string;
	initialHierarchyLevel?: string;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SearchBar({
	repositories,
	searchLoading,
	onSearch,
	onClear,
	hasResults,
	initialQuery = "",
	initialMode = "Hybrid",
	initialRepoId = "",
	initialLanguage = "",
	initialFilePattern = "",
	initialHierarchyLevel = "",
}: SearchBarProps) {
	const [query, setQuery] = useState(initialQuery);
	const [mode, setMode] = useState<CodeSearchMode>(initialMode);
	const [repoId, setRepoId] = useState<string>(initialRepoId);
	const [language, setLanguage] = useState(initialLanguage);
	const [filePattern, setFilePattern] = useState(initialFilePattern);
	const [hierarchyLevel, setHierarchyLevel] = useState(initialHierarchyLevel);
	const [limit, setLimit] = useState(20);

	const canSubmit = query.trim().length > 0 && !searchLoading;

	function handleSubmit(e: React.FormEvent) {
		e.preventDefault();
		if (!canSubmit) return;

		const req: CodeSearchRequest = {
			query: query.trim(),
			search_mode: mode,
			limit,
			repo_id: repoId || undefined,
			language: language.trim() || undefined,
			file_pattern: filePattern.trim() || undefined,
			hierarchy_level: hierarchyLevel || undefined,
		};
		void onSearch(req);
	}

	function handleClear() {
		setQuery("");
		onClear();
	}

	return (
		<form onSubmit={handleSubmit} className="space-y-3">
			{/* Main search row */}
			<div className="flex gap-2">
				<div className="relative flex-1">
					<Search
						size={16}
						className="absolute left-3 top-1/2 -translate-y-1/2 text-th-text-muted pointer-events-none"
						aria-hidden="true"
					/>
					<input
						type="search"
						value={query}
						onChange={(e) => setQuery(e.target.value)}
						placeholder="Search code…"
						className="w-full rounded-md border border-th-border-input bg-th-input pl-9 pr-4 py-2 text-sm text-th-text placeholder:text-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
					/>
				</div>

				{/* Mode selector */}
				<select
					value={mode}
					onChange={(e) => setMode(e.target.value as CodeSearchMode)}
					aria-label="Search mode"
					className="rounded-md border border-th-border-input bg-th-input px-3 py-2 text-sm text-th-text focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
				>
					{SEARCH_MODES.map((m) => (
						<option key={m.value} value={m.value}>
							{m.label}
						</option>
					))}
				</select>

				{/* Submit */}
				<button
					type="submit"
					disabled={!canSubmit}
					className="rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
				>
					{searchLoading ? "Searching…" : "Search"}
				</button>

				{/* Clear results */}
				{hasResults && (
					<button
						type="button"
						onClick={handleClear}
						aria-label="Clear results"
						className="rounded-md border border-th-border-strong bg-th-surface px-3 py-2 text-sm text-th-text-muted hover:text-th-text hover:bg-th-surface-hover transition-colors"
					>
						<X size={16} />
					</button>
				)}
			</div>

			{/* Secondary filters row */}
			<div className="flex flex-wrap gap-2 items-center">
				{/* Repository filter */}
				{repositories.length > 0 && (
					<select
						value={repoId}
						onChange={(e) => setRepoId(e.target.value)}
						aria-label="Filter by repository"
						className="rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
					>
						<option value="">All repositories</option>
						{repositories.map((r) => (
							<option key={r.id} value={r.id}>
								{r.name}
							</option>
						))}
					</select>
				)}

				{/* Language filter */}
				<input
					type="text"
					value={language}
					onChange={(e) => setLanguage(e.target.value)}
					placeholder="Language (e.g. rust)"
					aria-label="Language filter"
					className="rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text-secondary placeholder:text-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-focus-ring w-40"
				/>

				{/* File pattern filter */}
				<input
					type="text"
					value={filePattern}
					onChange={(e) => setFilePattern(e.target.value)}
					placeholder="File pattern (e.g. src/**)"
					aria-label="File pattern filter"
					className="rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text-secondary placeholder:text-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-focus-ring w-44"
				/>

				{/* Hierarchy level filter */}
				<select
					value={hierarchyLevel}
					onChange={(e) => setHierarchyLevel(e.target.value)}
					aria-label="Hierarchy level filter"
					className="rounded-md border border-th-border-input bg-th-input px-3 py-1.5 text-xs text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
				>
					{HIERARCHY_LEVELS.map((h) => (
						<option key={h.value} value={h.value}>
							{h.label}
						</option>
					))}
				</select>

				{/* Limit */}
				<div className="flex items-center gap-1.5">
					<label className="text-xs text-th-text-muted">Limit</label>
					<select
						value={limit}
						onChange={(e) => setLimit(Number(e.target.value))}
						aria-label="Result limit"
						className="rounded-md border border-th-border-input bg-th-input px-2 py-1.5 text-xs text-th-text-secondary focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
					>
						{[10, 20, 50, 100].map((n) => (
							<option key={n} value={n}>
								{n}
							</option>
						))}
					</select>
				</div>
			</div>
		</form>
	);
}

export default SearchBar;
