/**
 * useIndexService — state management for the Index service code search and
 * repository management workflow.
 *
 * Provides:
 * - Service health (reachable, checking, version)
 * - Repository list: add, delete, reindex with busy-state tracking
 * - Code search: vector/keyword/hybrid with results, loading, error states
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { indexClient } from "@/services/codeindex";
import type {
	AddRepoRequest,
	CodeSearchRequest,
	CodeSearchResultItem,
	EmbeddingSamplePoint,
	RepoRecord,
} from "@/types/codeindex";
import type { HealthResponse } from "@/types/common";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface IndexServiceHealth {
	reachable: boolean;
	checking: boolean;
	version?: string;
}

export interface UseIndexServiceResult {
	// Health
	health: IndexServiceHealth;
	recheckHealth: () => void;

	// Repositories
	repositories: RepoRecord[];
	reposLoading: boolean;
	reposError?: string;
	repoBusyIds: Set<string>;
	addRepository: (req: AddRepoRequest) => Promise<boolean>;
	deleteRepository: (id: string) => Promise<boolean>;
	reindexRepository: (id: string) => Promise<boolean>;
	refetchRepos: () => void;

	// Search
	searchResults: CodeSearchResultItem[];
	searchTotal: number;
	searchLoading: boolean;
	searchError?: string;
	searchQueryMs?: number;
	runSearch: (req: CodeSearchRequest) => Promise<void>;
	clearSearch: () => void;

	// Embedding sample
	embeddingPoints: EmbeddingSamplePoint[];
	embeddingTotal: number;
	embeddingSampled: number;
	embeddingLoading: boolean;
	embeddingError?: string;
	fetchEmbeddingSample: (repoId: string, limit?: number) => Promise<void>;
	clearEmbeddingSample: () => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useIndexService(): UseIndexServiceResult {
	const [health, setHealth] = useState<IndexServiceHealth>({
		reachable: false,
		checking: true,
	});
	const [repositories, setRepositories] = useState<RepoRecord[]>([]);
	const [reposLoading, setReposLoading] = useState(true);
	const [reposError, setReposError] = useState<string | undefined>();
	const [repoBusyIds, setRepoBusyIds] = useState<Set<string>>(new Set());

	const [searchResults, setSearchResults] = useState<CodeSearchResultItem[]>([]);
	const [searchTotal, setSearchTotal] = useState(0);
	const [searchLoading, setSearchLoading] = useState(false);
	const [searchError, setSearchError] = useState<string | undefined>();
	const [searchQueryMs, setSearchQueryMs] = useState<number | undefined>();

	const [embeddingPoints, setEmbeddingPoints] = useState<EmbeddingSamplePoint[]>([]);
	const [embeddingTotal, setEmbeddingTotal] = useState(0);
	const [embeddingSampled, setEmbeddingSampled] = useState(0);
	const [embeddingLoading, setEmbeddingLoading] = useState(false);
	const [embeddingError, setEmbeddingError] = useState<string | undefined>();

	const mountedRef = useRef(true);

	// -------------------------------------------------------------------------
	// Health check
	// -------------------------------------------------------------------------

	const recheckHealth = useCallback(async () => {
		setHealth((prev) => ({ ...prev, checking: true }));
		try {
			const res = (await indexClient.getHealth()) as HealthResponse;
			if (!mountedRef.current) return;
			setHealth({ reachable: true, checking: false, version: res.version });
		} catch {
			if (!mountedRef.current) return;
			setHealth({ reachable: false, checking: false });
		}
	}, []);

	useEffect(() => {
		void recheckHealth();
	}, [recheckHealth]);

	// -------------------------------------------------------------------------
	// Fetch repositories
	// -------------------------------------------------------------------------

	const fetchRepos = useCallback(async () => {
		if (!mountedRef.current) return;
		setReposLoading(true);
		setReposError(undefined);
		try {
			const res = await indexClient.listRepositories();
			if (!mountedRef.current) return;
			setRepositories(res.repositories);
		} catch (err) {
			if (!mountedRef.current) return;
			setReposError(
				err instanceof Error ? err.message : "Failed to load repositories",
			);
		} finally {
			if (mountedRef.current) setReposLoading(false);
		}
	}, []);

	useEffect(() => {
		mountedRef.current = true;
		void fetchRepos();
		return () => {
			mountedRef.current = false;
		};
	}, [fetchRepos]);

	// -------------------------------------------------------------------------
	// Busy state helpers
	// -------------------------------------------------------------------------

	const setRepoBusy = (id: string, busy: boolean) => {
		setRepoBusyIds((prev) => {
			const next = new Set(prev);
			if (busy) next.add(id);
			else next.delete(id);
			return next;
		});
	};

	// -------------------------------------------------------------------------
	// Repository actions
	// -------------------------------------------------------------------------

	const addRepository = useCallback(
		async (req: AddRepoRequest): Promise<boolean> => {
			try {
				const repo = await indexClient.addRepository(req);
				if (!mountedRef.current) return false;
				setRepositories((prev) => [...prev, repo]);
				return true;
			} catch (err) {
				setReposError(
					err instanceof Error ? err.message : "Failed to add repository",
				);
				return false;
			}
		},
		[],
	);

	const deleteRepository = useCallback(async (id: string): Promise<boolean> => {
		setRepoBusy(id, true);
		try {
			await indexClient.deleteRepository(id);
			if (!mountedRef.current) return false;
			setRepositories((prev) => prev.filter((r) => r.id !== id));
			return true;
		} catch (err) {
			setReposError(
				err instanceof Error ? err.message : "Failed to delete repository",
			);
			return false;
		} finally {
			setRepoBusy(id, false);
		}
	}, []);

	const reindexRepository = useCallback(
		async (id: string): Promise<boolean> => {
			setRepoBusy(id, true);
			try {
				const updated = await indexClient.reindexRepository(id);
				if (!mountedRef.current) return false;
				setRepositories((prev) =>
					prev.map((r) => (r.id === id ? updated : r)),
				);
				return true;
			} catch (err) {
				setReposError(
					err instanceof Error ? err.message : "Failed to trigger reindex",
				);
				return false;
			} finally {
				setRepoBusy(id, false);
			}
		},
		[],
	);

	// -------------------------------------------------------------------------
	// Search
	// -------------------------------------------------------------------------

	const runSearch = useCallback(async (req: CodeSearchRequest): Promise<void> => {
		setSearchLoading(true);
		setSearchError(undefined);
		try {
			const res = await indexClient.search(req);
			if (!mountedRef.current) return;
			setSearchResults(res.results);
			setSearchTotal(res.total);
			setSearchQueryMs(res.query_time_ms);
		} catch (err) {
			if (!mountedRef.current) return;
			setSearchError(
				err instanceof Error ? err.message : "Search failed",
			);
			setSearchResults([]);
			setSearchTotal(0);
		} finally {
			if (mountedRef.current) setSearchLoading(false);
		}
	}, []);

	const clearSearch = useCallback(() => {
		setSearchResults([]);
		setSearchTotal(0);
		setSearchError(undefined);
		setSearchQueryMs(undefined);
	}, []);

	// -------------------------------------------------------------------------
	// Embedding sample
	// -------------------------------------------------------------------------

	const fetchEmbeddingSample = useCallback(
		async (repoId: string, limit = 500): Promise<void> => {
			setEmbeddingLoading(true);
			setEmbeddingError(undefined);
			try {
				const res = await indexClient.getEmbeddingSample(repoId, limit);
				if (!mountedRef.current) return;
				if (!res || !Array.isArray(res.points)) {
					setEmbeddingError("Unexpected response from server — try again");
					return;
				}
				setEmbeddingPoints(res.points);
				setEmbeddingTotal(res.total_chunks ?? 0);
				setEmbeddingSampled(res.sampled ?? res.points.length);
			} catch (err) {
				if (!mountedRef.current) return;
				setEmbeddingError(
					err instanceof Error ? err.message : "Failed to fetch embedding sample",
				);
				setEmbeddingPoints([]);
				setEmbeddingTotal(0);
				setEmbeddingSampled(0);
			} finally {
				if (mountedRef.current) setEmbeddingLoading(false);
			}
		},
		[],
	);

	const clearEmbeddingSample = useCallback(() => {
		setEmbeddingPoints([]);
		setEmbeddingTotal(0);
		setEmbeddingSampled(0);
		setEmbeddingError(undefined);
	}, []);

	return {
		health,
		recheckHealth,
		repositories,
		reposLoading,
		reposError,
		repoBusyIds,
		addRepository,
		deleteRepository,
		reindexRepository,
		refetchRepos: () => { void fetchRepos(); },
		searchResults,
		searchTotal,
		searchLoading,
		searchError,
		searchQueryMs,
		runSearch,
		clearSearch,
		embeddingPoints,
		embeddingTotal,
		embeddingSampled,
		embeddingLoading,
		embeddingError,
		fetchEmbeddingSample,
		clearEmbeddingSample,
	};
}
