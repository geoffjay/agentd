/**
 * MSW request handlers for the Notify service (port 17004).
 *
 * Provides default responses for all Notify API endpoints.
 * Override per test with server.use().
 */

import { HttpResponse, http } from "msw";
import type { PaginatedResponse } from "@/types/common";
import type { Notification } from "@/types/notify";
import { makeCountResponse, makeNotificationList } from "../factories";

const BASE = "http://localhost:17004";

const DEFAULT_NOTIFICATIONS = makeNotificationList(3);
const DEFAULT_PENDING = makeNotificationList(2, {
	status: "pending",
	requires_response: true,
});

function paginated<T>(items: T[], total?: number): PaginatedResponse<T> {
	return { items, total: total ?? items.length, limit: 20, offset: 0 };
}

export const notifyHandlers = [
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	http.get(`${BASE}/health`, () =>
		HttpResponse.json({ status: "ok", service: "notify", version: "0.2.0" }),
	),

	// -------------------------------------------------------------------------
	// Notifications CRUD
	// -------------------------------------------------------------------------

	http.get(`${BASE}/notifications`, () =>
		HttpResponse.json(paginated<Notification>(DEFAULT_NOTIFICATIONS)),
	),

	http.post(`${BASE}/notifications`, async ({ request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		const notif = makeNotificationList(1, {
			title: String(body.title ?? "New Notification"),
		})[0];
		return HttpResponse.json(notif, { status: 201 });
	}),

	// -------------------------------------------------------------------------
	// Filtered views + count
	//
	// NOTE: these MUST be registered before the `/notifications/:id` routes —
	// MSW matches handlers in order, and `:id` would otherwise swallow
	// `actionable`, `history`, and `count` as path parameters.
	// -------------------------------------------------------------------------

	http.get(`${BASE}/notifications/actionable`, () =>
		HttpResponse.json(paginated<Notification>(DEFAULT_PENDING)),
	),

	http.get(`${BASE}/notifications/history`, () =>
		HttpResponse.json(paginated<Notification>([])),
	),

	http.get(`${BASE}/notifications/count`, () =>
		HttpResponse.json(
			makeCountResponse(DEFAULT_NOTIFICATIONS.length, {
				Pending: 2,
				Viewed: 1,
			}),
		),
	),

	// -------------------------------------------------------------------------
	// Single-notification routes
	// -------------------------------------------------------------------------

	http.get(`${BASE}/notifications/:id`, ({ params }) => {
		const notif =
			DEFAULT_NOTIFICATIONS.find((n) => n.id === params.id) ??
			makeNotificationList(1, { id: String(params.id) })[0];
		return HttpResponse.json(notif);
	}),

	http.put(`${BASE}/notifications/:id`, async ({ params, request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		const existing =
			DEFAULT_NOTIFICATIONS.find((n) => n.id === params.id) ??
			makeNotificationList(1, { id: String(params.id) })[0];
		return HttpResponse.json({ ...existing, ...body });
	}),

	http.delete(
		`${BASE}/notifications/:id`,
		() => new HttpResponse(null, { status: 204 }),
	),
];
