/**
 * Integration test for useAgentSummary hook.
 *
 * Uses MSW to test loading, success, and error states.
 */

import { describe, it, expect } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { useAgentSummary } from '@/hooks/useAgentSummary'
import { makeAgentList } from '@/test/mocks/factories'

describe('useAgentSummary (MSW integration)', () => {
  it('returns loading=true initially', () => {
    const { result } = renderHook(() => useAgentSummary())
    expect(result.current.loading).toBe(true)
  })

  it('returns correct status counts after load', async () => {
    server.use(
      http.get('http://localhost:17006/agents', () =>
        HttpResponse.json({
          items: [
            ...makeAgentList(2, { status: 'Running' }),
            ...makeAgentList(1, { status: 'Failed' }),
            ...makeAgentList(1, { status: 'Stopped' }),
          ],
          total: 4,
          limit: 200,
          offset: 0,
        }),
      ),
    )

    const { result } = renderHook(() => useAgentSummary())

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.counts.running).toBe(2)
    expect(result.current.counts.failed).toBe(1)
    expect(result.current.counts.stopped).toBe(1)
    expect(result.current.total).toBe(4)
  })

  it('returns the 5 most recently updated agents', async () => {
    const agents = makeAgentList(8)
    // Give them different timestamps
    agents.forEach((a, i) => {
      a.updated_at = new Date(Date.now() - i * 60_000).toISOString()
    })

    server.use(
      http.get('http://localhost:17006/agents', () =>
        HttpResponse.json({ items: agents, total: agents.length, limit: 200, offset: 0 }),
      ),
    )

    const { result } = renderHook(() => useAgentSummary())

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.recentAgents).toHaveLength(5)
    // First agent (most recently updated) should be first
    expect(result.current.recentAgents[0].id).toBe(agents[0].id)
  })

  it('sets error state when the API call fails', async () => {
    server.use(
      http.get('http://localhost:17006/agents', () =>
        HttpResponse.json({ error: 'Internal Server Error' }, { status: 500 }),
      ),
    )

    const { result } = renderHook(() => useAgentSummary())

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBeDefined()
  })
})
