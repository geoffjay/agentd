/**
 * useActivityFeed -- derivation unit tests (events + hourly buckets) and an
 * MSW integration smoke test.
 */

import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
	activityFeedInternals,
	useActivityFeed,
} from "@/hooks/useActivityFeed";
import {
	makeAgent,
	makeNotification,
	makeQuestion,
} from "@/test/mocks/factories";

const { buildEvents, buildBuckets } = activityFeedInternals;

const NOW = new Date("2024-06-15T12:30:00Z");

describe("buildEvents", () => {
	it("merges notifications, questions, and agents sorted newest first", () => {
		const events = buildEvents(
			[makeNotification({ id: "n1", created_at: "2024-06-15T10:00:00Z" })],
			[makeQuestion({ id: "q1", asked_at: "2024-06-15T12:00:00Z" })],
			[makeAgent({ id: "a1", updated_at: "2024-06-15T11:00:00Z" })],
		);

		expect(events.map((e) => e.type)).toEqual([
			"question",
			"agent",
			"notification",
		]);
	});

	it("caps the merged feed at 12 events", () => {
		const notifications = Array.from({ length: 20 }, (_, i) =>
			makeNotification({ id: `n${i}`, created_at: "2024-06-15T10:00:00Z" }),
		);
		expect(buildEvents(notifications, [], [])).toHaveLength(12);
	});

	it("includes title and message in notification descriptions", () => {
		const [event] = buildEvents(
			[makeNotification({ title: "Build failed", message: "exit code 1" })],
			[],
			[],
		);
		expect(event.description).toContain("Build failed");
		expect(event.description).toContain("exit code 1");
	});

	it("drops events with invalid timestamps", () => {
		const events = buildEvents(
			[makeNotification({ created_at: "not-a-date" })],
			[],
			[],
		);
		expect(events).toHaveLength(0);
	});
});

describe("buildBuckets", () => {
	it("returns 24 hourly buckets", () => {
		const buckets = buildBuckets([], [], [], NOW);
		expect(buckets).toHaveLength(24);
		expect(buckets.every((b) => /^\d{2}:00$/.test(b.hour))).toBe(true);
	});

	it("counts items into the correct hour bucket by type", () => {
		const oneHourAgo = new Date(NOW.getTime() - 60 * 60 * 1000).toISOString();
		const buckets = buildBuckets(
			[makeNotification({ created_at: oneHourAgo })],
			[makeQuestion({ asked_at: oneHourAgo })],
			[makeAgent({ updated_at: NOW.toISOString() })],
			NOW,
		);

		// Second-to-last bucket = previous hour; last bucket = current hour
		expect(buckets[22].notifications).toBe(1);
		expect(buckets[22].questions).toBe(1);
		expect(buckets[23].agents).toBe(1);
	});

	it("ignores items outside the 24h window", () => {
		const twoDaysAgo = new Date(
			NOW.getTime() - 48 * 60 * 60 * 1000,
		).toISOString();
		const buckets = buildBuckets(
			[makeNotification({ created_at: twoDaysAgo })],
			[],
			[],
			NOW,
		);
		expect(buckets.every((b) => b.notifications === 0)).toBe(true);
	});
});

describe("useActivityFeed (MSW integration)", () => {
	it("loads events from the default handlers without errors", async () => {
		const { result } = renderHook(() => useActivityFeed());

		await waitFor(() => expect(result.current.loading).toBe(false));

		expect(result.current.error).toBeUndefined();
		expect(result.current.buckets).toHaveLength(24);
		// Default handlers provide notifications, questions, and agents
		expect(result.current.events.length).toBeGreaterThan(0);
	});
});
