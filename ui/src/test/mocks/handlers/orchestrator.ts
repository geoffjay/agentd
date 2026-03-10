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
    const agent =
      DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json(agent)
  }),

  http.delete(`${BASE}/agents/:id`, ({ params }) => {
    const agent =
      DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json(agent)
  }),

  // -------------------------------------------------------------------------
  // Agent usage
  // -------------------------------------------------------------------------

  http.get(`${BASE}/agents/:id/usage`, ({ params }) =>
    HttpResponse.json({
      agent_id: String(params.id),
      current_session: {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_input_tokens: 20,
        cache_creation_input_tokens: 10,
        total_cost_usd: 0.01,
        num_turns: 2,
        duration_ms: 1000,
        duration_api_ms: 800,
        result_count: 2,
        started_at: '2024-01-01T00:00:00Z',
      },
      cumulative: {
        input_tokens: 200,
        output_tokens: 100,
        cache_read_input_tokens: 40,
        cache_creation_input_tokens: 20,
        total_cost_usd: 0.02,
        num_turns: 4,
        duration_ms: 2000,
        duration_api_ms: 1600,
        result_count: 4,
        started_at: '2024-01-01T00:00:00Z',
      },
      session_count: 1,
    }),
  ),

  // -------------------------------------------------------------------------
  // Agent context management
  // -------------------------------------------------------------------------

  http.post(`${BASE}/agents/:id/clear-context`, ({ params }) =>
    HttpResponse.json({
      agent_id: String(params.id),
      new_session_number: 2,
      session_usage: {
        input_tokens: 100,
        output_tokens: 50,
        cache_read_input_tokens: 20,
        cache_creation_input_tokens: 10,
        total_cost_usd: 0.01,
        num_turns: 2,
        duration_ms: 1000,
        duration_api_ms: 800,
        result_count: 2,
        started_at: '2024-01-01T00:00:00Z',
      },
    }),
  ),

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
  // Agent model
  // -------------------------------------------------------------------------

  http.put(`${BASE}/agents/:id/model`, async ({ params, request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const agent =
      DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json({
      ...agent,
      config: { ...agent.config, model: body.model ?? agent.config.model },
    })
  }),

  // -------------------------------------------------------------------------
  // Tool policy
  // -------------------------------------------------------------------------

  http.get(`${BASE}/agents/:id/policy`, ({ params }) => {
    const agent =
      DEFAULT_AGENTS.find((a) => a.id === params.id) ?? makeAgent({ id: String(params.id) })
    return HttpResponse.json(agent.config.tool_policy)
  }),

  http.put(`${BASE}/agents/:id/policy`, async ({ request }) => {
    const policy = await request.json()
    return HttpResponse.json(policy)
  }),

  // -------------------------------------------------------------------------
  // Agent approvals
  // -------------------------------------------------------------------------

  http.get(`${BASE}/agents/:id/approvals`, () => HttpResponse.json(paginated<PendingApproval>([]))),

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
