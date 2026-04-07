/**
 * IndexMain — main Code Index page component.
 *
 * Layout:
 * - Page header with service health indicator and "Add Repository" button
 * - Two-tab view: Repositories | Search
 * - Repositories tab: RepositoryList with per-row reindex/delete actions
 * - Search tab: SearchBar + results summary; results rendered by
 *   SearchResultsTable (issue-1036) — falls back to a simple result count
 *   until that component is available.
 *
 * URL query params are synced for deep-linking and browser back/forward:
 *   tab             "repositories" | "search"
 *   q               search query string
 *   mode            "Hybrid" | "Vector" | "Keyword"
 *   repo            repository ID filter
 *   lang            language filter
 *   file_pattern    file glob filter
 *   hierarchy       hierarchy level filter
 *
 * All service state is managed by useIndexService().
 */

import {
	AlertCircle,
	CheckCircle,
	Database,
	Loader2,
	Plus,
	RefreshCw,
	Search,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { AddRepositoryDialog } from "@/components/index/AddRepositoryDialog";
import { RepositoryList } from "@/components/index/RepositoryList";
import { SearchBar } from "@/components/index/SearchBar";
import { useIndexService } from "@/hooks/useIndexService";
import type { CodeSearchMode } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Tab types
// ---------------------------------------------------------------------------

type Tab = "repositories" | "search";

// ---------------------------------------------------------------------------
// Health indicator
// ---------------------------------------------------------------------------

function HealthIndicator({
	reachable,
	checking,
	version,
}: {
	reachable: boolean;
	checking: boolean;
	version?: string;
}) {
	if (checking) {
		return (
			<span className="inline-flex items-center gap-1.5 text-xs text-th-text-muted">
				<Loader2 size={13} className="animate-spin" aria-hidden="true" />
				Checking service…
			</span>
		);
	}

	if (!reachable) {
		return (
			<span className="inline-flex items-center gap-1.5 text-xs text-th-status-error-text">
				<AlertCircle size={13} aria-hidden="true" />
				Index service unreachable
			</span>
		);
	}

	return (
		<span className="inline-flex items-center gap-1.5 text-xs text-th-status-success-text">
			<CheckCircle size={13} aria-hidden="true" />
			Service online{version ? ` · v${version}` : ""}
		</span>
	);
}

// ---------------------------------------------------------------------------
// Result summary row
// ---------------------------------------------------------------------------

function SearchSummary({
	total,
	queryMs,
}: {
	total: number;
	queryMs?: number;
}) {
	return (
		<p className="text-sm text-th-text-muted">
			{total} result{total !== 1 ? "s" : ""}
			{queryMs !== undefined && (
				<span className="ml-2 text-th-text-muted opacity-70">
					({queryMs} ms)
				</span>
			)}
		</p>
	);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function IndexMain() {
	const {
		health,
		recheckHealth,
		repositories,
		reposLoading,
		reposError,
		repoBusyIds,
		addRepository,
		deleteRepository,
		reindexRepository,
		refetchRepos,
		searchResults,
		searchTotal,
		searchLoading,
		searchError,
		searchQueryMs,
		runSearch,
		clearSearch,
	} = useIndexService();

	const [searchParams, setSearchParams] = useSearchParams();

	// ---------------------------------------------------------------------------
	// State initialised from URL params
	// ---------------------------------------------------------------------------

	const [activeTab, setActiveTab] = useState<Tab>(
		(searchParams.get("tab") as Tab | null) ?? "repositories",
	);
	const [showAddDialog, setShowAddDialog] = useState(false);

	// Search filter state — kept here so IndexMain can sync them to the URL.
	const [searchQuery, setSearchQuery] = useState(
		searchParams.get("q") ?? "",
	);
	const [searchMode, setSearchMode] = useState<CodeSearchMode>(
		(searchParams.get("mode") as CodeSearchMode | null) ?? "Hybrid",
	);
	const [searchRepoId, setSearchRepoId] = useState(
		searchParams.get("repo") ?? "",
	);
	const [searchLanguage, setSearchLanguage] = useState(
		searchParams.get("lang") ?? "",
	);
	const [searchFilePattern, setSearchFilePattern] = useState(
		searchParams.get("file_pattern") ?? "",
	);
	const [searchHierarchyLevel, setSearchHierarchyLevel] = useState(
		searchParams.get("hierarchy") ?? "",
	);

	// ---------------------------------------------------------------------------
	// Sync state -> URL params
	// ---------------------------------------------------------------------------

	useEffect(() => {
		const params: Record<string, string> = {};

		if (activeTab !== "repositories") params.tab = activeTab;
		if (searchQuery) params.q = searchQuery;
		if (searchMode !== "Hybrid") params.mode = searchMode;
		if (searchRepoId) params.repo = searchRepoId;
		if (searchLanguage) params.lang = searchLanguage;
		if (searchFilePattern) params.file_pattern = searchFilePattern;
		if (searchHierarchyLevel) params.hierarchy = searchHierarchyLevel;

		setSearchParams(params, { replace: true });
	}, [
		activeTab,
		searchQuery,
		searchMode,
		searchRepoId,
		searchLanguage,
		searchFilePattern,
		searchHierarchyLevel,
		setSearchParams,
	]);

	const hasResults = searchResults.length > 0;

	return (
		<div className="space-y-5">
			{/* Page header */}
			<div className="flex items-start justify-between gap-4">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">Code Index</h1>
					<div className="mt-1 flex items-center gap-3">
						<HealthIndicator
							reachable={health.reachable}
							checking={health.checking}
							version={health.version}
						/>
						{!health.checking && (
							<button
								type="button"
								onClick={recheckHealth}
								aria-label="Recheck service health"
								className="text-xs text-th-text-muted hover:text-th-text transition-colors"
							>
								<RefreshCw size={12} />
							</button>
						)}
					</div>
				</div>

				<button
					type="button"
					onClick={() => setShowAddDialog(true)}
					className="flex items-center gap-1.5 rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors"
				>
					<Plus size={16} aria-hidden="true" />
					Add Repository
				</button>
			</div>

			{/* Error banner */}
			{reposError && (
				<div
					role="alert"
					className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
				>
					<p>{reposError}</p>
					<button
						type="button"
						onClick={refetchRepos}
						className="mt-2 text-xs underline underline-offset-2 hover:opacity-80 transition-opacity"
					>
						Retry
					</button>
				</div>
			)}

			{/* Tab bar */}
			<div
				className="flex gap-1 rounded-lg border border-th-border bg-th-surface-sunken p-1"
				role="tablist"
				aria-label="Code Index views"
			>
				<button
					role="tab"
					type="button"
					aria-selected={activeTab === "repositories"}
					onClick={() => setActiveTab("repositories")}
					className={[
						"flex items-center gap-1.5 rounded-md px-4 py-1.5 text-sm font-medium transition-colors",
						activeTab === "repositories"
							? "bg-th-surface text-th-text shadow-sm"
							: "text-th-text-muted hover:text-th-text",
					].join(" ")}
				>
					<Database size={15} aria-hidden="true" />
					Repositories
					{repositories.length > 0 && (
						<span className="ml-1 rounded-full bg-th-surface-sunken px-1.5 py-0.5 text-[11px] font-medium text-th-text-muted">
							{repositories.length}
						</span>
					)}
				</button>

				<button
					role="tab"
					type="button"
					aria-selected={activeTab === "search"}
					onClick={() => setActiveTab("search")}
					className={[
						"flex items-center gap-1.5 rounded-md px-4 py-1.5 text-sm font-medium transition-colors",
						activeTab === "search"
							? "bg-th-surface text-th-text shadow-sm"
							: "text-th-text-muted hover:text-th-text",
					].join(" ")}
				>
					<Search size={15} aria-hidden="true" />
					Search
					{hasResults && (
						<span className="ml-1 rounded-full bg-th-accent px-1.5 py-0.5 text-[11px] font-medium text-th-accent-text">
							{searchTotal}
						</span>
					)}
				</button>
			</div>

			{/* ============================================================ */}
			{/* Repositories tab                                             */}
			{/* ============================================================ */}
			{activeTab === "repositories" && (
				<div className="space-y-3">
					<div className="flex items-center justify-between">
						<p className="text-sm text-th-text-muted">
							{repositories.length} repositor{repositories.length !== 1 ? "ies" : "y"} registered
						</p>
						<button
							type="button"
							onClick={refetchRepos}
							aria-label="Refresh repositories"
							className="rounded-md border border-th-border-strong bg-th-surface p-1.5 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary transition-colors"
						>
							<RefreshCw size={14} />
						</button>
					</div>

					<RepositoryList
						repositories={repositories}
						loading={reposLoading}
						busyIds={repoBusyIds}
						onReindex={reindexRepository}
						onDelete={deleteRepository}
					/>
				</div>
			)}

			{/* ============================================================ */}
			{/* Search tab                                                   */}
			{/* ============================================================ */}
			{activeTab === "search" && (
				<div className="space-y-4">
					<SearchBar
						repositories={repositories}
						searchLoading={searchLoading}
						onSearch={async (req) => {
							// Mirror filter state up so URL params stay in sync.
							setSearchQuery(req.query);
							setSearchMode(req.search_mode);
							setSearchRepoId(req.repo_id ?? "");
							setSearchLanguage(req.language ?? "");
							setSearchFilePattern(req.file_pattern ?? "");
							setSearchHierarchyLevel(req.hierarchy_level ?? "");
							await runSearch(req);
						}}
						onClear={() => {
							setSearchQuery("");
							clearSearch();
						}}
						hasResults={hasResults}
						initialQuery={searchQuery}
						initialMode={searchMode}
						initialRepoId={searchRepoId}
						initialLanguage={searchLanguage}
						initialFilePattern={searchFilePattern}
						initialHierarchyLevel={searchHierarchyLevel}
					/>

					{/* Search error */}
					{searchError && (
						<div
							role="alert"
							className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
						>
							{searchError}
						</div>
					)}

					{/* Results summary */}
					{hasResults && !searchLoading && (
						<SearchSummary total={searchTotal} queryMs={searchQueryMs} />
					)}

					{/* Results placeholder — SearchResultsTable (issue-1036) renders here */}
					{hasResults && (
						<div className="divide-y divide-th-border rounded-md border border-th-border">
							{searchResults.map((result) => (
								<div key={result.id} className="px-4 py-3 space-y-1">
									<div className="flex items-center justify-between gap-2">
										<span className="text-sm font-medium text-th-text font-mono">
											{result.file_path}
										</span>
										<span className="shrink-0 rounded-full bg-th-surface-sunken px-2 py-0.5 text-xs text-th-text-muted">
											{result.language}
										</span>
									</div>
									<p className="text-xs text-th-text-muted">
										Lines {result.start_line}–{result.end_line}
										{result.symbol_name && (
											<span className="ml-2 font-mono">{result.symbol_name}</span>
										)}
										<span className="ml-2 opacity-60">
											score {result.score.toFixed(3)}
										</span>
									</p>
									<pre className="mt-1 overflow-x-auto rounded bg-th-surface-sunken px-3 py-2 text-xs text-th-text-secondary whitespace-pre-wrap line-clamp-4">
										{result.content}
									</pre>
								</div>
							))}
						</div>
					)}

					{/* Empty search state */}
					{!searchLoading && !searchError && !hasResults && (
						<div className="flex flex-col items-center justify-center py-16 text-center">
							<Search
								size={36}
								className="text-th-text-muted opacity-40 mb-3"
								aria-hidden="true"
							/>
							<p className="text-sm font-medium text-th-text-muted">
								Search your indexed code
							</p>
							<p className="mt-1 text-xs text-th-text-muted opacity-70">
								Enter a query above to search across all indexed repositories.
							</p>
						</div>
					)}
				</div>
			)}

			{/* Add repository dialog */}
			<AddRepositoryDialog
				open={showAddDialog}
				onSave={addRepository}
				onClose={() => setShowAddDialog(false)}
			/>
		</div>
	);
}

export default IndexMain;
