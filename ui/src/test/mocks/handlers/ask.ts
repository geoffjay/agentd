/**
 * MSW request handlers for the Ask service (port 17001).
 *
 * Provides default responses for health and the new /questions/* endpoints.
 * Legacy /trigger and /answer handlers are kept for backwards compatibility
 * while useAskService is migrated (#1009).
 */

import { HttpResponse, http } from "msw";
import {
	makeAnswerResponse,
	makeQuestion,
	makeTriggerResponse,
} from "../factories";

const BASE = "http://localhost:17001";

export const askHandlers = [
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	http.get(`${BASE}/health`, () =>
		HttpResponse.json({ status: "ok", service: "ask", version: "0.12.0" }),
	),

	// -------------------------------------------------------------------------
	// Questions
	// -------------------------------------------------------------------------

	http.get(`${BASE}/questions`, () =>
		HttpResponse.json({
			questions: [makeQuestion(), makeQuestion({ status: "Answered" })],
			total: 2,
		}),
	),

	http.get(`${BASE}/questions/:id`, ({ params }) =>
		HttpResponse.json(makeQuestion({ id: String(params.id) })),
	),

	http.post(`${BASE}/questions`, async ({ request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		return HttpResponse.json(
			makeQuestion({ agent_id: String(body.agent_id ?? "agent-1") }),
			{ status: 201 },
		);
	}),

	http.post(`${BASE}/questions/:id/answer`, async ({ params, request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		return HttpResponse.json(
			makeQuestion({
				id: String(params.id),
				status: "Answered",
				answer: String(body.answer ?? ""),
			}),
		);
	}),

	http.post(`${BASE}/questions/:id/dismiss`, ({ params }) =>
		HttpResponse.json(
			makeQuestion({
				id: String(params.id),
				status: "Dismissed",
			}),
		),
	),

	// -------------------------------------------------------------------------
	// Legacy endpoints — kept while useAskService is being migrated (#1009)
	// -------------------------------------------------------------------------

	http.post(`${BASE}/trigger`, () => HttpResponse.json(makeTriggerResponse())),

	http.post(`${BASE}/answer`, async ({ request }) => {
		const body = (await request.json()) as Record<string, unknown>;
		return HttpResponse.json(
			makeAnswerResponse({ question_id: String(body.question_id ?? "1") }),
		);
	}),
];
