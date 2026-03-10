/**
 * Tests for OrchestratorClient usage and context management endpoints.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { OrchestratorClient } from '@/services/orchestrator'
import { ApiError } from '@/types/common'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeJsonResponse(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: new Headers({ 'content-type': 'application/json' }),
  })
}

function mockFetch(status: number, body: unknown) {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(makeJsonResponse(status, body)))
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const usageResponse = {
  agent_id: 'agent-uuid-1',
  current_session: {
    input_tokens: 500,
    output_tokens: 250,
    cache_read_input_tokens: 100,
    cache_creation_input_tokens: 50,
    total_cost_usd: 0.015,
    num_turns: 3,
    duration_ms: 8000,
    duration_api_ms: 5500,
    result_count: 3,
    started_at: '2024-06-15T10:00:00Z',
  },
  cumulative: {
    input_tokens: 2000,
    output_tokens: 1000,
    cache_read_input_tokens: 500,
    cache_creation_input_tokens: 200,
    total_cost_usd: 0.06,
    num_turns: 12,
    duration_ms: 35000,
    duration_api_ms: 25000,
    result_count: 12,
    started_at: '2024-06-10T08:00:00Z',
  },
  session_count: 3,
}

const clearContextResponse = {
  agent_id: 'agent-uuid-1',
  new_session_number: 4,
  session_usage: usageResponse.current_session,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('OrchestratorClient usage endpoints', () => {
  let client: OrchestratorClient

  beforeEach(() => {
    client = new OrchestratorClient({
      baseUrl: 'http://localhost:17006',
      maxRetries: 1,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  // -------------------------------------------------------------------------
  // getAgentUsage
  // -------------------------------------------------------------------------

  describe('getAgentUsage', () => {
    it('calls GET /agents/:id/usage and returns deserialized response', async () => {
      mockFetch(200, usageResponse)
      const result = await client.getAgentUsage('agent-uuid-1')

      expect(result.agent_id).toBe('agent-uuid-1')
      expect(result.current_session?.input_tokens).toBe(500)
      expect(result.cumulative.total_cost_usd).toBe(0.06)
      expect(result.session_count).toBe(3)

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/agents/agent-uuid-1/usage')
    })

    it('uses GET method', async () => {
      mockFetch(200, usageResponse)
      await client.getAgentUsage('agent-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('GET')
    })

    it('throws ApiError on 404', async () => {
      mockFetch(404, { error: 'Agent not found' })
      await expect(client.getAgentUsage('missing')).rejects.toMatchObject({
        status: 404,
        message: 'Agent not found',
      })
    })

    it('throws ApiError on 500', async () => {
      mockFetch(500, { error: 'Internal server error' })
      await expect(client.getAgentUsage('agent-uuid-1')).rejects.toMatchObject({
        status: 500,
        message: 'Internal server error',
      })
    })

    it('thrown error is an ApiError instance', async () => {
      mockFetch(500, { error: 'fail' })
      let caught: unknown = null
      try {
        await client.getAgentUsage('agent-uuid-1')
      } catch (e) {
        caught = e
      }
      expect(caught).toBeInstanceOf(ApiError)
    })
  })

  // -------------------------------------------------------------------------
  // clearContext
  // -------------------------------------------------------------------------

  describe('clearContext', () => {
    it('calls POST /agents/:id/clear-context and returns response', async () => {
      mockFetch(200, clearContextResponse)
      const result = await client.clearContext('agent-uuid-1')

      expect(result.agent_id).toBe('agent-uuid-1')
      expect(result.new_session_number).toBe(4)

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/agents/agent-uuid-1/clear-context')
    })

    it('uses POST method', async () => {
      mockFetch(200, clearContextResponse)
      await client.clearContext('agent-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('POST')
    })

    it('sends empty JSON body', async () => {
      mockFetch(200, clearContextResponse)
      await client.clearContext('agent-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      const body = JSON.parse(callInit.body as string)
      expect(body).toEqual({})
    })

    it('throws ApiError on 404', async () => {
      mockFetch(404, { error: 'Agent not found' })
      await expect(client.clearContext('missing')).rejects.toMatchObject({
        status: 404,
        message: 'Agent not found',
      })
    })

    it('throws ApiError on 500', async () => {
      mockFetch(500, { error: 'Internal server error' })
      await expect(client.clearContext('agent-uuid-1')).rejects.toMatchObject({
        status: 500,
        message: 'Internal server error',
      })
    })

    it('returns session_usage when provided', async () => {
      mockFetch(200, clearContextResponse)
      const result = await client.clearContext('agent-uuid-1')
      expect(result.session_usage?.input_tokens).toBe(500)
    })
  })
})
