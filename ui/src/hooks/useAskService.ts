/**
 * useAskService — state management for the Ask service Q&A workflow.
 *
 * Provides:
 * - Service health (reachability, version)
 * - Questions: paginated list with optional filters
 * - Actions: answer and dismiss pending questions
 * - Polling: auto-refresh configurable interval
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { askClient } from "@/services/ask";
import type { ListQuestionsParams, Question, QuestionStatus } from "@/types/ask";
import type { HealthResponse } from "@/types/common";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AskServiceHealth {
	reachable: boolean;
	checking: boolean;
	version?: string;
}

export type PollingInterval = 5_000 | 15_000 | 30_000 | 60_000;

export const POLLING_INTERVAL_OPTIONS: PollingInterval[] = [
	5_000, 15_000, 30_000, 60_000,
];

export interface UseAskServiceOptions {
	/** Initial filter params passed to GET /questions */
	params?: ListQuestionsParams;
	/** Polling interval in ms; 0 = disabled (default 15 000) */
	pollingInterval?: number;
}

export interface UseAskServiceResult {
	// Health
	health: AskServiceHealth;
	recheckHealth: () => void;

	// Questions list
	questions: Question[];
	total: number;
	loading: boolean;
	error?: string;

	// Filters
	filters: ListQuestionsParams;
	setFilters: (f: ListQuestionsParams) => void;
	setStatusFilter: (status: QuestionStatus | undefined) => void;

	// Actions
	busyIds: Set<string>;
	answerQuestion: (id: string, answer: string) => Promise<boolean>;
	dismissQuestion: (id: string) => Promise<boolean>;
	actionError?: string;

	// Polling
	pollingEnabled: boolean;
	pollingInterval: PollingInterval;
	setPollingEnabled: (enabled: boolean) => void;
	setPollingInterval: (ms: PollingInterval) => void;

	// Manual refresh
	refetch: () => void;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const DEFAULT_POLLING: PollingInterval = 15_000;

export function useAskService({
	params: initialParams = {},
	pollingInterval: initialPollingInterval = DEFAULT_POLLING,
}: UseAskServiceOptions = {}): UseAskServiceResult {
	const [health, setHealth] = useState<AskServiceHealth>({
		reachable: false,
		checking: true,
	});
	const [questions, setQuestions] = useState<Question[]>([]);
	const [total, setTotal] = useState(0);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | undefined>();
	const [filters, setFiltersState] = useState<ListQuestionsParams>(
		initialParams,
	);
	const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
	const [actionError, setActionError] = useState<string | undefined>();
	const [pollingEnabled, setPollingEnabled] = useState(false);
	const [pollingInterval, setPollingInterval] =
		useState<PollingInterval>(initialPollingInterval as PollingInterval);

	const mountedRef = useRef(true);
	const pollingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

	// -------------------------------------------------------------------------
	// Health check
	// -------------------------------------------------------------------------

	const recheckHealth = useCallback(async () => {
		setHealth((prev) => ({ ...prev, checking: true }));
		try {
			const res = (await askClient.getHealth()) as HealthResponse;
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
	// Fetch questions
	// -------------------------------------------------------------------------

	const fetchQuestions = useCallback(
		async (showLoading = true) => {
			if (!mountedRef.current) return;
			if (showLoading) {
				setLoading(true);
				setError(undefined);
			}
			try {
				const result = await askClient.listQuestions(filters);
				if (!mountedRef.current) return;
				setQuestions(result.items);
				setTotal(result.total);
				setError(undefined);
			} catch (err) {
				if (!mountedRef.current) return;
				setError(
					err instanceof Error ? err.message : "Failed to load questions",
				);
			} finally {
				if (mountedRef.current) setLoading(false);
			}
		},
		[filters],
	);

	useEffect(() => {
		mountedRef.current = true;
		void fetchQuestions(true);
		return () => {
			mountedRef.current = false;
		};
	}, [fetchQuestions]);

	// -------------------------------------------------------------------------
	// Auto-enable polling when pending questions exist
	// -------------------------------------------------------------------------

	useEffect(() => {
		const hasPending = questions.some((q) => q.status === "Pending");
		setPollingEnabled(hasPending);
	}, [questions]);

	// -------------------------------------------------------------------------
	// Polling timer
	// -------------------------------------------------------------------------

	useEffect(() => {
		if (pollingTimerRef.current) clearInterval(pollingTimerRef.current);
		if (!pollingEnabled || !pollingInterval) return;
		pollingTimerRef.current = setInterval(
			() => void fetchQuestions(false),
			pollingInterval,
		);
		return () => {
			if (pollingTimerRef.current) clearInterval(pollingTimerRef.current);
		};
	}, [pollingEnabled, pollingInterval, fetchQuestions]);

	// -------------------------------------------------------------------------
	// Filters
	// -------------------------------------------------------------------------

	const setFilters = useCallback((f: ListQuestionsParams) => {
		setFiltersState(f);
	}, []);

	const setStatusFilter = useCallback((status: QuestionStatus | undefined) => {
		setFiltersState((prev) => ({ ...prev, status }));
	}, []);

	// -------------------------------------------------------------------------
	// Busy state helpers
	// -------------------------------------------------------------------------

	const setBusy = (id: string, busy: boolean) => {
		setBusyIds((prev) => {
			const next = new Set(prev);
			if (busy) next.add(id);
			else next.delete(id);
			return next;
		});
	};

	const updateLocal = (id: string, patch: Partial<Question>) => {
		setQuestions((prev) =>
			prev.map((q) => (q.id === id ? { ...q, ...patch } : q)),
		);
	};

	// -------------------------------------------------------------------------
	// Actions
	// -------------------------------------------------------------------------

	const answerQuestion = useCallback(
		async (id: string, answer: string): Promise<boolean> => {
			setBusy(id, true);
			setActionError(undefined);
			try {
				const updated = await askClient.answerQuestion(id, { answer });
				// Backend returns the updated Question — apply it directly.
				updateLocal(id, { status: updated.status, answer: updated.answer ?? answer });
				return true;
			} catch (err) {
				setActionError(
					err instanceof Error ? err.message : "Failed to submit answer",
				);
				return false;
			} finally {
				setBusy(id, false);
			}
		},
		[],
	);

	const dismissQuestion = useCallback(async (id: string): Promise<boolean> => {
		setBusy(id, true);
		setActionError(undefined);
		try {
			const updated = await askClient.dismissQuestion(id);
			// Backend returns the updated Question — apply it directly.
			updateLocal(id, { status: updated.status });
			return true;
		} catch (err) {
			setActionError(
				err instanceof Error ? err.message : "Failed to dismiss question",
			);
			return false;
		} finally {
			setBusy(id, false);
		}
	}, []);

	return {
		health,
		recheckHealth,
		questions,
		total,
		loading,
		error,
		filters,
		setFilters,
		setStatusFilter,
		busyIds,
		answerQuestion,
		dismissQuestion,
		actionError,
		pollingEnabled,
		pollingInterval,
		setPollingEnabled,
		setPollingInterval,
		refetch: () => {
			void fetchQuestions(false);
		},
	};
}
