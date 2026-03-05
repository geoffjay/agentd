/**
 * MSW request handlers for the Orchestrator service (port 17006).
 *
 * These provide default responses for all Orchestrator API endpoints.
 * Override individual handlers per test using server.use().
 */

import { http, HttpResponse } from 'msw'
import { makeAgent, makeAgentList, makeApprovalList } from '../factories'
import type { PaginatedResponse } from '@/types/common'
import type { Agent, PendingApproval } from '@/types/orchestrator'

const BASE = 'http://localhost:17006'

// Default dataset shared by all handlers (reset per test via factories)
const DEFAULT_AGENTS = makeAgentList(3)
const DEFAULT_APPROVALS = makeApprovalList(1)

function paginated<T>(items: T[], total?: number): PaginatedResponse<T> {
  return { items, total: total ?? items.length, limit: 20, offset: 0 }
}

export const orchestratorHandlers = [
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  http.get(`${BASE}/health`, () =>
    HttpResponse.json({ status: 'ok', service: 'orchestrator', version: '0.2.0' }),
  ),

  // -------------------------------------------------------------------------
  // Agents
  // -------------------------------------------------------------------------

  http.get(`${BASE}/agents`, () => HttpResponse.json(paginated<Agent>(DEFAULT_AGENTS))),

  http.post(`${BASE}/agents`, async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const agent = makeAgent({ name: String(body.name ?? 'new-agent') })
    return HttpResponse.json(agent, { status: 201 })
  }),

  http.get(`${BASE}/agents/:id`, ({ params }) => {
    const agent = DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json(agent)
  }),

  http.delete(`${BASE}/agents/:id`, ({ params }) => {
    const agent = DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json(agent)
  }),

  // -------------------------------------------------------------------------
  // Agent actions
  // -------------------------------------------------------------------------

  http.post(`${BASE}/agents/:id/message`, () =>
    HttpResponse.json({ status: 'sent', agent_id: '1' }),
  ),

  http.post(`${BASE}/agents/:id/start`, ({ params }) =>
    HttpResponse.json(makeAgent({ id: String(params.id), status: 'Running' })),
  ),

  http.post(`${BASE}/agents/:id/stop`, ({ params }) =>
    HttpResponse.json(makeAgent({ id: String(params.id), status: 'Stopped' })),
  ),

  http.post(`${BASE}/agents/:id/restart`, ({ params }) =>
    HttpResponse.json(makeAgent({ id: String(params.id), status: 'Running' })),
  ),

  // -------------------------------------------------------------------------
  // Approvals
  // -------------------------------------------------------------------------

  http.get(`${BASE}/approvals`, () =>
    HttpResponse.json(paginated<PendingApproval>(DEFAULT_APPROVALS)),
  ),

  http.post(`${BASE}/approvals/:id/approve`, ({ params }) =>
    HttpResponse.json({ id: params.id, status: 'Approved' }),
  ),

  http.post(`${BASE}/approvals/:id/deny`, ({ params }) =>
    HttpResponse.json({ id: params.id, status: 'Denied' }),
  ),
]
