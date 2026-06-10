/**
 * useDashboardStats — fetches the overview stat counts that are not already
 * provided by useAgentSummary / useNotificationSummary:
 *
 * - Pending tool-use approvals (orchestrator)
 * - Workflows (orchestrator)
 * - Pending questions (ask)
 *
 * Each source is fetched with Promise.allSettled, so a dead service nulls
 * out only its own stat (rendered as "—") without breaking the others.
 */

import { useCallback, useRef, useState } from "react";
import { askClient } from "@/services/ask";
import { orchestratorClient } from "@/services/orchestrator";
import { usePolling } from "./usePolling";

export interface UseDashboardStatsResult {
	/** Pending approvals count (null when the orchestrator is unreachable) */
	pendingApprovals: number | null;
	/** Total workflow count (null when the orchestrator is unreachable) */
	workflows: number | null;
	/** Pending questions count (null when the ask service is unreachable) */
	pendingQuestions: number | null;
	/** True only during the very first load */
	loading: boolean;
	refetch: () => void;
}

export function useDashboardStats(): UseDashboardStatsResult {
	const [pendingApprovals, setPendingApprovals] = useState<number | null>(null);
	const [workflows, setWorkflows] = useState<number | null>(null);
	const [pendingQuestions, setPendingQuestions] = useState<number | null>(null);
	const [loading, setLoading] = useState(true);
	const hasLoadedRef = useRef(false);

	const fetch = useCallback(async () => {
		if (!hasLoadedRef.current) setLoading(true);
		const [approvalsRes, workflowsRes, questionsRes] = await Promise.allSettled(
			[
				orchestratorClient.listApprovals({ status: "pending", limit: 1 }),
				orchestratorClient.listWorkflows({ limit: 1 }),
				askClient.listQuestions({ status: "Pending", limit: 1 }),
			],
		);

		setPendingApprovals(
			approvalsRes.status === "fulfilled" ? approvalsRes.value.total : null,
		);
		setWorkflows(
			workflowsRes.status === "fulfilled" ? workflowsRes.value.total : null,
		);
		setPendingQuestions(
			questionsRes.status === "fulfilled" ? questionsRes.value.total : null,
		);

		hasLoadedRef.current = true;
		setLoading(false);
	}, []);

	usePolling(fetch);

	const refetch = useCallback(() => {
		void fetch();
	}, [fetch]);

	return { pendingApprovals, workflows, pendingQuestions, loading, refetch };
}
