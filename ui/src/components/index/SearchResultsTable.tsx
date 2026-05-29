/**
 * SearchResultsTable — DataTable for code search results with a code preview
 * drawer on row click.
 *
 * Columns: Score, Symbol, File, Language, Type, Lines
 *
 * Wraps itself in a DrawerProvider so it owns the drawer lifecycle. The
 * CodePreviewDrawer is rendered inside the drawer for the selected result.
 */

import { useCallback } from "react";
import type { ColumnDef } from "@/components/common";
import { DataTable, DrawerProvider, useDrawer } from "@/components/common";
import { CodePreviewDrawer } from "@/components/index/CodePreviewDrawer";
import type { CodeSearchResultItem } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface SearchResultsTableProps {
	results: CodeSearchResultItem[];
	loading: boolean;
}

// ---------------------------------------------------------------------------
// Score pill
// ---------------------------------------------------------------------------

function ScorePill({ score }: { score: number }) {
	const pct = score * 100;
	const colorClass =
		pct >= 80
			? "bg-th-status-success-bg text-th-status-success-text"
			: pct >= 50
				? "bg-th-status-warning-bg text-th-status-warning-text"
				: "bg-th-surface-sunken text-th-text-muted";

	return (
		<span
			className={[
				"inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium tabular-nums",
				colorClass,
			].join(" ")}
		>
			{pct.toFixed(0)}%
		</span>
	);
}

// ---------------------------------------------------------------------------
// Inner table (needs drawer context)
// ---------------------------------------------------------------------------

function SearchResultsTableInner({
	results,
	loading,
}: SearchResultsTableProps) {
	const { openDrawer } = useDrawer();

	const handleRowClick = useCallback(
		(result: CodeSearchResultItem) => {
			const title =
				result.symbol_name ??
				result.file_path.split("/").pop() ??
				result.file_path;
			openDrawer(title, <CodePreviewDrawer result={result} />);
		},
		[openDrawer],
	);

	const columns: ColumnDef<CodeSearchResultItem>[] = [
		{
			key: "score",
			header: "Score",
			headerClassName: "w-16",
			render: (r) => <ScorePill score={r.score} />,
		},
		{
			key: "symbol_name",
			header: "Symbol",
			render: (r) =>
				r.symbol_name ? (
					<span className="text-sm font-mono text-th-text">
						{r.symbol_name}
					</span>
				) : (
					<span className="text-xs text-th-text-muted">—</span>
				),
		},
		{
			key: "file_path",
			header: "File",
			render: (r) => {
				const parts = r.file_path.split("/");
				const file = parts.pop() ?? r.file_path;
				const dir = parts.join("/");
				return (
					<span className="text-sm font-mono">
						{dir && <span className="text-th-text-muted">{dir}/</span>}
						<span className="text-th-text">{file}</span>
					</span>
				);
			},
		},
		{
			key: "language",
			header: "Lang",
			headerClassName: "w-20",
			render: (r) => (
				<span className="rounded bg-th-surface-sunken px-1.5 py-0.5 text-xs font-mono text-th-text-secondary">
					{r.language}
				</span>
			),
		},
		{
			key: "chunk_type",
			header: "Type",
			headerClassName: "w-24",
			render: (r) => (
				<span className="text-xs text-th-text-muted">{r.chunk_type}</span>
			),
		},
		{
			key: "lines",
			header: "Lines",
			headerClassName: "w-20",
			render: (r) => (
				<span className="text-xs tabular-nums text-th-text-muted whitespace-nowrap">
					{r.start_line}–{r.end_line}
				</span>
			),
		},
	];

	return (
		<DataTable
			columns={columns}
			data={results}
			rowKey={(r) => r.id}
			loading={loading}
			onRowClick={handleRowClick}
			emptyTitle="No results"
			emptyDescription="Try a different query or search mode."
		/>
	);
}

// ---------------------------------------------------------------------------
// Exported component (wraps with DrawerProvider)
// ---------------------------------------------------------------------------

export function SearchResultsTable(props: SearchResultsTableProps) {
	return (
		<DrawerProvider>
			<SearchResultsTableInner {...props} />
		</DrawerProvider>
	);
}

export default SearchResultsTable;
