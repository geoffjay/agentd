/**
 * useActivityFeed — fetches notifications, questions, and agents once per
 * poll tick and derives two views from the same data:
 *
 * 1. `events` — merged recent-activity list for the ActivityTimeline
 *    (sorted newest-first, capped at 12 entries).
 * 2. `buckets` — hourly activity counts for the last 24 hours, stacked by
 *    type, for the ActivityChart.
 *
 * Sources are fetched with Promise.allSettled so a single dead service
 * degrades gracefully instead of blanking the whole feed.
 */

import { useCallback, useRef, useState } from "react";
import type { ActivityEvent } from "@/components/dashboard/ActivityTimeline";
import { askClient } from "@/services/ask";
import { notifyClient } from "@/services/notify";
import { orchestratorClient } from "@/services/orchestrator";
import type { Question } from "@/types/ask";
import type { Notification } from "@/types/notify";
import type { Agent } from "@/types/orchestrator";
import { usePolling } from "./usePolling";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Activity counts for a single hour bucket */
export interface ActivityBucket {
	/** Hour label, e.g. "13:00" */
	hour: string;
	notifications: number;
	questions: number;
	agents: number;
}

export interface UseActivityFeedResult {
	/** Merged recent events, newest first, capped at 12 */
	events: ActivityEvent[];
	/** Hourly activity buckets covering the last 24 hours, oldest first */
	buckets: ActivityBucket[];
	/** True only during the very first load */
	loading: boolean;
	error?: string;
	refetch: () => void;
}

const MAX_EVENTS = 12;
const BUCKET_COUNT = 24;
const HOUR_MS = 60 * 60 * 1000;

// ---------------------------------------------------------------------------
// Derivation helpers
// ---------------------------------------------------------------------------

function buildEvents(
	notifications: Notification[],
	questions: Question[],
	agents: Agent[],
): ActivityEvent[] {
	const events: ActivityEvent[] = [];

	for (const n of notifications) {
		events.push({
			id: `notification-${n.id}`,
			type: "notification",
			description: n.message ? `${n.title} — ${n.message}` : n.title,
			timestamp: new Date(n.created_at),
		});
	}

	for (const q of questions) {
		events.push({
			id: `question-${q.id}`,
			type: "question",
			description: `Question ${q.status.toLowerCase()}: ${q.question}`,
			timestamp: new Date(q.asked_at),
		});
	}

	for (const a of agents) {
		events.push({
			id: `agent-${a.id}`,
			type: "agent",
			description: `Agent "${a.name}" is ${a.status.toLowerCase()}`,
			timestamp: new Date(a.updated_at),
		});
	}

	return events
		.filter((e) => !Number.isNaN(e.timestamp.getTime()))
		.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime())
		.slice(0, MAX_EVENTS);
}

function buildBuckets(
	notifications: Notification[],
	questions: Question[],
	agents: Agent[],
	now: Date = new Date(),
): ActivityBucket[] {
	// Bucket 0 starts 23 hours before the start of the current hour.
	const currentHourStart = new Date(now);
	currentHourStart.setMinutes(0, 0, 0);
	const windowStart = currentHourStart.getTime() - (BUCKET_COUNT - 1) * HOUR_MS;

	const buckets: ActivityBucket[] = Array.from(
		{ length: BUCKET_COUNT },
		(_, i) => {
			const start = new Date(windowStart + i * HOUR_MS);
			return {
				hour: `${String(start.getHours()).padStart(2, "0")}:00`,
				notifications: 0,
				questions: 0,
				agents: 0,
			};
		},
	);

	const add = (
		timestamp: string,
		key: "notifications" | "questions" | "agents",
	) => {
		const time = new Date(timestamp).getTime();
		if (Number.isNaN(time)) return;
		const index = Math.floor((time - windowStart) / HOUR_MS);
		if (index >= 0 && index < BUCKET_COUNT) buckets[index][key]++;
	};

	for (const n of notifications) add(n.created_at, "notifications");
	for (const q of questions) add(q.asked_at, "questions");
	for (const a of agents) add(a.updated_at, "agents");

	return buckets;
}

export const activityFeedInternals = { buildEvents, buildBuckets };

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useActivityFeed(): UseActivityFeedResult {
	const [events, setEvents] = useState<ActivityEvent[]>([]);
	const [buckets, setBuckets] = useState<ActivityBucket[]>([]);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | undefined>();
	const hasLoadedRef = useRef(false);

	const fetch = useCallback(async () => {
		if (!hasLoadedRef.current) setLoading(true);
		const [actionableRes, historyRes, questionsRes, agentsRes] =
			await Promise.allSettled([
				notifyClient.listActionable({ limit: 100 }),
				notifyClient.listHistory({ limit: 100 }),
				askClient.listQuestions({ limit: 100 }),
				orchestratorClient.listAgents({ limit: 200 }),
			]);

		// Merge actionable + history notifications, de-duplicated by ID.
		// Guard against unexpected payload shapes — never crash the dashboard.
		const notificationsById = new Map<string, Notification>();
		for (const res of [actionableRes, historyRes]) {
			if (res.status === "fulfilled") {
				for (const n of res.value.items ?? []) notificationsById.set(n.id, n);
			}
		}
		const notifications = [...notificationsById.values()];
		const questions =
			questionsRes.status === "fulfilled"
				? (questionsRes.value.items ?? [])
				: [];
		const agents =
			agentsRes.status === "fulfilled" ? (agentsRes.value.items ?? []) : [];

		const allFailed = [
			actionableRes,
			historyRes,
			questionsRes,
			agentsRes,
		].every((r) => r.status === "rejected");

		setError(allFailed ? "Failed to load recent activity" : undefined);
		setEvents(buildEvents(notifications, questions, agents));
		setBuckets(buildBuckets(notifications, questions, agents));
		hasLoadedRef.current = true;
		setLoading(false);
	}, []);

	usePolling(fetch);

	const refetch = useCallback(() => {
		void fetch();
	}, [fetch]);

	return { events, buckets, loading, error, refetch };
}
