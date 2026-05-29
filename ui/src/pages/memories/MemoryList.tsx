/**
 * MemoryList — full memory management page.
 *
 * Features:
 * - Tab toggle between Browse (list) and Semantic Search modes
 * - Filter by type, visibility, creator, tag, and content search
 * - Sort by created_at, updated_at, type
 * - Paginated table of memories
 * - Create memory dialog
 * - Delete confirmation dialog
 * - Drawer for memory details on row click
 * - URL query param sync for filters/sort/pagination
 * - Loading skeleton, error state, and empty state
 */

import {
	Globe,
	List,
	Lock,
	Plus,
	RefreshCw,
	Search,
	Users,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import type { ColumnDef } from "@/components/common";
import { DataTable, DrawerProvider, useDrawer } from "@/components/common";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { Pagination } from "@/components/common/Pagination";
import { CreateMemoryDialog } from "@/components/memories/CreateMemoryDialog";
import { MemoryDetail } from "@/components/memories/MemoryDetail";
import { MemoryFilters } from "@/components/memories/MemoryFilters";
import { MemorySearch } from "@/components/memories/MemorySearch";
import {
	type MemoryFilters as MemoryFiltersType,
	type MemorySortDir,
	type MemorySortField,
	useMemories,
} from "@/hooks/useMemories";
import type { Memory, VisibilityLevel } from "@/types/memory";

// ---------------------------------------------------------------------------
// View mode
// ---------------------------------------------------------------------------

type ViewMode = "list" | "search";

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
// URL sync helpers
// ---------------------------------------------------------------------------

function filtersFromParams(p: URLSearchParams): MemoryFiltersType {
	return {
		type: (p.get("type") as MemoryFiltersType["type"]) || undefined,
		visibility:
			(p.get("visibility") as MemoryFiltersType["visibility"]) || undefined,
		created_by: p.get("created_by") || undefined,
		tag: p.get("tag") || undefined,
	};
}

function sortFieldFromParam(p: string | null): MemorySortField {
	if (p === "updated_at" || p === "type") return p;
	return "created_at";
}

function sortDirFromParam(p: string | null): MemorySortDir {
	if (p === "asc") return "asc";
	return "desc";
}

function viewModeFromParam(p: string | null): ViewMode {
	if (p === "search") return "search";
	return "list";
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_PAGE_SIZE = 20;

// ---------------------------------------------------------------------------
// Inner page (needs drawer context)
// ---------------------------------------------------------------------------

function MemoryListInner() {
	const [searchParams, setSearchParams] = useSearchParams();
	const { openDrawer, closeDrawer } = useDrawer();

	// View mode
	const [viewMode, setViewModeState] = useState<ViewMode>(() =>
		viewModeFromParam(searchParams.get("view")),
	);

	// State initialised from URL params
	const [filters, setFiltersState] = useState<MemoryFiltersType>(() =>
		filtersFromParams(searchParams),
	);
	const [search, setSearchState] = useState(
		() => searchParams.get("search") || "",
	);
	const [sortBy, setSortByState] = useState<MemorySortField>(() =>
		sortFieldFromParam(searchParams.get("sortBy")),
	);
	const [sortDir, setSortDirState] = useState<MemorySortDir>(() =>
		sortDirFromParam(searchParams.get("sortDir")),
	);
	const [page, setPageState] = useState(
		() => Number(searchParams.get("page")) || 1,
	);

	// Sync state → URL
	useEffect(() => {
		const params: Record<string, string> = {};
		if (viewMode !== "list") params["view"] = viewMode;
		if (filters.type) params["type"] = filters.type;
		if (filters.visibility) params["visibility"] = filters.visibility;
		if (filters.created_by) params["created_by"] = filters.created_by;
		if (filters.tag) params["tag"] = filters.tag;
		if (search) params["search"] = search;
		if (sortBy !== "created_at") params["sortBy"] = sortBy;
		if (sortDir !== "desc") params["sortDir"] = sortDir;
		if (page > 1) params["page"] = String(page);
		setSearchParams(params, { replace: true });
	}, [viewMode, filters, search, sortBy, sortDir, page, setSearchParams]);

	// Reset page to 1 when filters change
	const setFilters = (f: MemoryFiltersType) => {
		setFiltersState(f);
		setPageState(1);
	};
	const setSearch = (s: string) => {
		setSearchState(s);
		setPageState(1);
	};
	const setSort = (field: MemorySortField, dir: MemorySortDir) => {
		setSortByState(field);
		setSortDirState(dir);
		setPageState(1);
	};
	const setViewMode = (mode: ViewMode) => {
		setViewModeState(mode);
		setPageState(1);
	};

	// Hook
	const {
		memories,
		total,
		loading,
		refreshing,
		error,
		refetch,
		createMemory,
		deleteMemory,
	} = useMemories({
		filters,
		search,
		page,
		pageSize: DEFAULT_PAGE_SIZE,
		sortBy,
		sortDir,
		paused: viewMode === "search",
	});

	const totalPages = Math.ceil(total / DEFAULT_PAGE_SIZE);

	// Create dialog state
	const [showCreateDialog, setShowCreateDialog] = useState(false);

	// Delete confirmation state
	const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
	const [deleteLoading, setDeleteLoading] = useState(false);

	const handleDeleteConfirm = async () => {
		if (!deleteTarget) return;
		setDeleteLoading(true);
		try {
			await deleteMemory(deleteTarget);
		} finally {
			setDeleteLoading(false);
			setDeleteTarget(null);
		}
	};

	// Edit visibility — placeholder for future dialog
	const handleEditVisibility = (_memory: Memory) => {
		// TODO: Open visibility edit dialog (future issue)
	};

	// Row click → open drawer
	const handleRowClick = useCallback(
		(m: Memory) => {
			openDrawer(
				m.content.length > 50 ? m.content.slice(0, 50) + "…" : m.content,
				<MemoryDetail
					memory={m}
					onEditVisibility={(mem) => {
						closeDrawer();
						handleEditVisibility(mem);
					}}
					onDelete={(id) => {
						closeDrawer();
						setDeleteTarget(id);
					}}
				/>,
			);
		},
		[openDrawer, closeDrawer],
	);

	// Handle sort from DataTable
	const handleTableSort = useCallback(
		(field: string) => {
			if (field === sortBy) {
				setSort(field as MemorySortField, sortDir === "asc" ? "desc" : "asc");
			} else {
				setSort(field as MemorySortField, "desc");
			}
		},
		[sortBy, sortDir],
	);

	// Column definitions
	const columns: ColumnDef<Memory>[] = [
		{
			key: "type",
			header: "Type",
			render: (m) => (
				<span
					className={[
						"inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium",
						TYPE_STYLES[m.type] ?? "bg-th-surface-sunken text-th-text-muted",
					].join(" ")}
				>
					{TYPE_LABELS[m.type] ?? m.type}
				</span>
			),
		},
		{
			key: "content",
			header: "Content",
			render: (m) => (
				<span className="text-sm text-th-text-secondary line-clamp-2">
					{m.content}
				</span>
			),
		},
		{
			key: "visibility",
			header: "Visibility",
			render: (m) => (
				<span
					className={[
						"inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium",
						VISIBILITY_STYLES[m.visibility] ??
							"bg-th-surface-sunken text-th-text-muted",
					].join(" ")}
				>
					{VISIBILITY_ICONS[m.visibility]}
					{m.visibility}
				</span>
			),
		},
		{
			key: "tags",
			header: "Tags",
			render: (m) =>
				m.tags.length > 0 ? (
					<div className="flex flex-wrap gap-1">
						{m.tags.slice(0, 3).map((tag) => (
							<span
								key={tag}
								className="rounded-full bg-th-surface-sunken px-2 py-0.5 text-xs font-medium text-th-text-secondary"
							>
								{tag}
							</span>
						))}
						{m.tags.length > 3 && (
							<span className="text-xs text-th-text-muted">
								+{m.tags.length - 3}
							</span>
						)}
					</div>
				) : (
					<span className="text-xs text-th-text-muted">—</span>
				),
		},
		{
			key: "created_by",
			header: "Creator",
			render: (m) => (
				<span className="text-sm text-th-text-muted">{m.created_by}</span>
			),
		},
		{
			key: "created_at",
			header: "Created",
			sortable: true,
			render: (m) => (
				<span className="text-sm text-th-text-muted whitespace-nowrap">
					{new Date(m.created_at).toLocaleDateString()}
				</span>
			),
		},
	];

	return (
		<div className="space-y-5">
			{/* Page header */}
			<div className="flex items-center justify-between">
				<div>
					<h1 className="text-2xl font-semibold text-th-text">Memories</h1>
					<p className="mt-1 text-sm text-th-text-muted">
						Manage stored knowledge and context.
						{viewMode === "list" && total > 0 && (
							<span className="ml-2 text-th-text-muted">({total} total)</span>
						)}
					</p>
				</div>

				<div className="flex items-center gap-2">
					{/* View mode toggle */}
					<div
						className="flex rounded-md border border-th-border-strong"
						role="group"
						aria-label="View mode"
					>
						<button
							type="button"
							onClick={() => setViewMode("list")}
							aria-pressed={viewMode === "list"}
							className={[
								"flex items-center gap-1.5 rounded-l-md px-3 py-1.5 text-xs font-medium transition-colors",
								viewMode === "list"
									? "bg-th-accent text-th-accent-text"
									: "bg-th-surface text-th-text-muted hover:text-th-text",
							].join(" ")}
						>
							<List size={14} aria-hidden="true" />
							Browse
						</button>
						<button
							type="button"
							onClick={() => setViewMode("search")}
							aria-pressed={viewMode === "search"}
							className={[
								"flex items-center gap-1.5 rounded-r-md px-3 py-1.5 text-xs font-medium transition-colors",
								viewMode === "search"
									? "bg-th-accent text-th-accent-text"
									: "bg-th-surface text-th-text-muted hover:text-th-text",
							].join(" ")}
						>
							<Search size={14} aria-hidden="true" />
							Search
						</button>
					</div>

					{/* Create button */}
					<button
						type="button"
						onClick={() => setShowCreateDialog(true)}
						className="flex items-center gap-1.5 rounded-md bg-th-accent px-4 py-2 text-sm font-medium text-th-accent-text hover:bg-th-accent-hover transition-colors"
					>
						<Plus size={16} aria-hidden="true" />
						New Memory
					</button>

					{/* Refresh (list mode only) */}
					{viewMode === "list" && (
						<button
							type="button"
							onClick={refetch}
							aria-label="Refresh memories"
							className={[
								"rounded-md border border-th-border-strong bg-th-surface p-2 text-th-text-muted hover:bg-th-surface-hover hover:text-th-text-secondary transition-colors",
								refreshing ? "animate-spin" : "",
							].join(" ")}
						>
							<RefreshCw size={16} />
						</button>
					)}
				</div>
			</div>

			{/* ============================================================ */}
			{/* Search mode                                                  */}
			{/* ============================================================ */}
			{viewMode === "search" && (
				<MemorySearch
					onSwitchToList={() => setViewMode("list")}
					onEditVisibility={handleEditVisibility}
					onDelete={setDeleteTarget}
				/>
			)}

			{/* ============================================================ */}
			{/* List mode                                                    */}
			{/* ============================================================ */}
			{viewMode === "list" && (
				<>
					{/* Filters row */}
					<MemoryFilters
						filters={filters}
						sortBy={sortBy}
						sortDir={sortDir}
						search={search}
						onFiltersChange={setFilters}
						onSortChange={setSort}
						onSearchChange={setSearch}
					/>

					{/* Error banner */}
					{error && (
						<div
							role="alert"
							className="rounded-md bg-th-status-error-bg px-4 py-3 text-sm text-th-status-error-text"
						>
							<p>{error}</p>
							<button
								type="button"
								onClick={refetch}
								className="mt-2 rounded-md px-3 py-1 text-xs font-medium bg-th-status-error-bg text-th-status-error-text hover:opacity-90 transition-colors"
							>
								Retry
							</button>
						</div>
					)}

					{/* Table */}
					<DataTable
						columns={columns}
						data={memories}
						rowKey={(m) => m.id}
						loading={loading}
						sortBy={sortBy}
						sortDir={sortDir}
						onSort={handleTableSort}
						onRowClick={handleRowClick}
						emptyTitle="No memories found"
						emptyDescription={
							search ||
							filters.type ||
							filters.visibility ||
							filters.tag ||
							filters.created_by
								? "Try adjusting your filters or search query."
								: "Get started by creating your first memory."
						}
					/>

					{/* Pagination */}
					{!loading && totalPages > 1 && (
						<Pagination
							page={page}
							totalPages={totalPages}
							totalItems={total}
							pageSize={DEFAULT_PAGE_SIZE}
							onPageChange={setPageState}
						/>
					)}
				</>
			)}

			{/* Create memory dialog */}
			<CreateMemoryDialog
				open={showCreateDialog}
				onSave={createMemory}
				onClose={() => setShowCreateDialog(false)}
			/>

			{/* Delete confirmation dialog */}
			<ConfirmDialog
				open={deleteTarget !== null}
				title="Delete memory"
				description="Are you sure you want to delete this memory? This action cannot be undone."
				confirmLabel="Delete"
				variant="danger"
				loading={deleteLoading}
				onConfirm={handleDeleteConfirm}
				onCancel={() => setDeleteTarget(null)}
			/>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Exported page (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function MemoryList() {
	return (
		<DrawerProvider>
			<MemoryListInner />
		</DrawerProvider>
	);
}

export default MemoryList;
