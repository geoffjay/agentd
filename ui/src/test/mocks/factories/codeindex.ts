/**
 * Test data factories for the Index service types.
 *
 * Usage:
 *   const repo = makeRepoRecord()
 *   const repo = makeRepoRecord({ status: 'Error' })
 *   const repos = makeRepoList(3)
 *   const result = makeSearchResultItem()
 *   const response = makeCodeSearchResponse([result1, result2])
 */

import type {
	AddRepoRequest,
	CodeAgenticMatch,
	CodeAgenticSearchResponse,
	CodeSearchResponse,
	CodeSearchResultItem,
	ListReposResponse,
	RepoRecord,
	RepoStatus,
	RepoStatusResponse,
} from "@/types/codeindex";

let _repoSeq = 0;
let _resultSeq = 0;

export function resetCodeIndexSeq(): void {
	_repoSeq = 0;
	_resultSeq = 0;
}

// ---------------------------------------------------------------------------
// RepoRecord
// ---------------------------------------------------------------------------

export function makeRepoRecord(overrides?: Partial<RepoRecord>): RepoRecord {
	const id = ++_repoSeq;
	return {
		id: `repo-${id}`,
		name: `repo-${id}`,
		path: `/projects/repo-${id}`,
		status: "ready" as RepoStatus,
		created_at: "2024-01-15T10:00:00.000Z",
		updated_at: "2024-01-15T10:00:00.000Z",
		last_indexed: "2024-01-15T11:00:00.000Z",
		...overrides,
	};
}

export function makeRepoList(
	count: number,
	overrides?: Partial<RepoRecord>,
): RepoRecord[] {
	return Array.from({ length: count }, () => makeRepoRecord(overrides));
}

export function makeListReposResponse(repos?: RepoRecord[]): ListReposResponse {
	const items = repos ?? makeRepoList(3);
	return { repositories: items, total: items.length };
}

export function makeRepoStatusResponse(
	overrides?: Partial<RepoStatusResponse>,
): RepoStatusResponse {
	return {
		id: "repo-1",
		status: "ready" as RepoStatus,
		last_indexed: "2024-01-15T11:00:00.000Z",
		...overrides,
	};
}

export function makeAddRepoRequest(
	overrides?: Partial<AddRepoRequest>,
): AddRepoRequest {
	const id = _repoSeq + 1;
	return {
		name: `new-repo-${id}`,
		path: `/projects/new-repo-${id}`,
		...overrides,
	};
}

// ---------------------------------------------------------------------------
// CodeSearchResultItem
// ---------------------------------------------------------------------------

export function makeSearchResultItem(
	overrides?: Partial<CodeSearchResultItem>,
): CodeSearchResultItem {
	const id = ++_resultSeq;
	return {
		id: `result-${id}`,
		file_path: `src/module-${id}/lib.rs`,
		language: "rust",
		chunk_type: "function",
		symbol_name: `function_${id}`,
		start_line: (id - 1) * 20 + 1,
		end_line: (id - 1) * 20 + 20,
		content: `pub fn function_${id}() -> String {\n    String::from("hello")\n}`,
		summary: `Function ${id} that returns a greeting string.`,
		score: 0.85,
		repo_id: "repo-1",
		...overrides,
	};
}

export function makeSearchResultList(
	count: number,
	overrides?: Partial<CodeSearchResultItem>,
): CodeSearchResultItem[] {
	return Array.from({ length: count }, () => makeSearchResultItem(overrides));
}

export function makeCodeSearchResponse(
	results?: CodeSearchResultItem[],
	overrides?: Partial<CodeSearchResponse>,
): CodeSearchResponse {
	const items = results ?? makeSearchResultList(3);
	return {
		results: items,
		total: items.length,
		query_time_ms: 42,
		...overrides,
	};
}

// ---------------------------------------------------------------------------
// Agentic search
// ---------------------------------------------------------------------------

export function makeAgenticMatch(
	overrides?: Partial<CodeAgenticMatch>,
): CodeAgenticMatch {
	return {
		file_path: "src/main.rs",
		line_number: 10,
		content: 'println!("hello world");',
		context_before: ["fn main() {"],
		context_after: ["}"],
		...overrides,
	};
}

export function makeAgenticSearchResponse(
	matches?: CodeAgenticMatch[],
): CodeAgenticSearchResponse {
	const items = matches ?? [makeAgenticMatch()];
	return { matches: items, total: items.length, query_time_ms: 15 };
}
