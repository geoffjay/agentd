/**
 * MSW request handlers for the Ask service (port 17001).
 *
 * Provides default responses for health, trigger, and answer endpoints.
 */

import { http, HttpResponse } from 'msw'
import { makeTriggerResponse, makeAnswerResponse } from '../factories'

const BASE = 'http://localhost:17001'

export const askHandlers = [
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  http.get(`${BASE}/health`, () =>
    HttpResponse.json({ status: 'ok', service: 'ask', version: '0.1.0' }),
  ),

  // -------------------------------------------------------------------------
  // Trigger
  // -------------------------------------------------------------------------

  http.post(`${BASE}/trigger`, () => HttpResponse.json(makeTriggerResponse())),

  // -------------------------------------------------------------------------
  // Answer
  // -------------------------------------------------------------------------

  http.post(`${BASE}/answer`, async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>
    return HttpResponse.json(
      makeAnswerResponse({ question_id: String(body.question_id ?? '1') }),
    )
  }),
]
