/**
 * CodePreviewDrawer — drawer content for a single code search result.
 *
 * Displays:
 * - File path, language, chunk type, score
 * - Line range and optional symbol name
 * - Repository ID
 * - Optional AI-generated summary
 * - Syntax-highlighted code via HighlightedCode
 */

import { HighlightedCode } from "@/components/common";
import type { CodeSearchResultItem } from "@/types/codeindex";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface CodePreviewDrawerProps {
	result: CodeSearchResultItem;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Map common language names to Shiki language identifiers. */
function toShikiLang(language: string): string {
	const map: Record<string, string> = {
		rust: "rust",
		typescript: "typescript",
		javascript: "javascript",
		python: "python",
		go: "go",
		ruby: "ruby",
		c: "c",
		cpp: "cpp",
		"c++": "cpp",
		java: "java",
		kotlin: "kotlin",
		swift: "swift",
		shell: "bash",
		bash: "bash",
		sh: "bash",
		toml: "toml",
		yaml: "yaml",
		json: "json",
		markdown: "markdown",
		md: "markdown",
		html: "html",
		css: "css",
	};
	return map[language.toLowerCase()] ?? "text";
}

// ---------------------------------------------------------------------------
// Meta row helper
// ---------------------------------------------------------------------------

function MetaRow({ label, value }: { label: string; value: React.ReactNode }) {
	return (
		<div className="flex items-start gap-2 py-1.5 border-b border-th-border last:border-0">
			<span className="w-28 shrink-0 text-xs text-th-text-muted">{label}</span>
			<span className="text-xs text-th-text break-all">{value}</span>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function CodePreviewDrawer({ result }: CodePreviewDrawerProps) {
	const scorePercent = (result.score * 100).toFixed(1);

	return (
		<div className="space-y-5">
			{/* Metadata table */}
			<div className="rounded-md border border-th-border bg-th-surface-sunken px-4 py-1">
				<MetaRow label="File" value={<span className="font-mono">{result.file_path}</span>} />
				<MetaRow label="Language" value={result.language} />
				<MetaRow label="Chunk type" value={result.chunk_type} />
				{result.symbol_name && (
					<MetaRow label="Symbol" value={<span className="font-mono">{result.symbol_name}</span>} />
				)}
				<MetaRow
					label="Lines"
					value={`${result.start_line} – ${result.end_line}`}
				/>
				<MetaRow
					label="Score"
					value={
						<span className="inline-flex items-center gap-1.5">
							<span
								className="inline-block h-1.5 rounded-full bg-th-accent"
								style={{ width: `${Math.round(result.score * 64)}px` }}
								aria-hidden="true"
							/>
							{scorePercent}%
						</span>
					}
				/>
				<MetaRow label="Repository" value={<span className="font-mono">{result.repo_id}</span>} />
			</div>

			{/* Summary */}
			{result.summary && (
				<div className="space-y-1">
					<h3 className="text-xs font-semibold uppercase tracking-wide text-th-text-muted">
						Summary
					</h3>
					<p className="text-sm text-th-text-secondary leading-relaxed">
						{result.summary}
					</p>
				</div>
			)}

			{/* Code */}
			<div className="space-y-1">
				<h3 className="text-xs font-semibold uppercase tracking-wide text-th-text-muted">
					Content
				</h3>
				<HighlightedCode
					code={result.content}
					language={toShikiLang(result.language)}
					maxHeight="28rem"
					className="border border-th-border"
				/>
			</div>
		</div>
	);
}

export default CodePreviewDrawer;
