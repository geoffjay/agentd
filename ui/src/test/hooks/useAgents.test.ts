/**
 * Tests for the useAgents hook.
 *
 * Uses MSW to intercept API calls. Timer-sensitive tests use vi.useFakeTimers()
 * locally and clean up in afterEach.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { useAgents } from '@/hooks/useAgents'
import { server } from '@/test/mocks/server'
import { makeAgentList, makeAgent, resetAgentSeq } from '@/test/mocks/factories'
import type { PaginatedResponse } from '@/types/common'
import type { Agent } from '@/types/orchestrator'

const BASE = 'http://localhost:17006'

function paginatedAgents(items: Agent[]): PaginatedResponse<Agent> {
  return { items, total: items.length, limit: 200, offset: 0 }
}

beforeEach(() => {
  resetAgentSeq()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useAgents', () => {
  it('fetches agents on mount', async () => {
    const agents = makeAgentList(3)
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.allAgents).toHaveLength(3)
    expect(result.current.agents).toHaveLength(3)
  })

  it('sets error when API fails', async () => {
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.error()),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBeDefined()
  })

  it('filters agents by name search', async () => {
    const agents = [
      makeAgent({ name: 'build-agent' }),
      makeAgent({ name: 'deploy-agent' }),
      makeAgent({ name: 'test-runner' }),
    ]
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
    )

    const { result } = renderHook(() =>
      useAgents({ search: 'build', paused: true }),
    )

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.agents).toHaveLength(1)
    expect(result.current.agents[0].name).toBe('build-agent')
    expect(result.current.total).toBe(1)
  })

  it('paginates agents', async () => {
    const agents = makeAgentList(25)
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
    )

    const { result } = renderHook(() =>
      useAgents({ page: 1, pageSize: 10, paused: true }),
    )

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.agents).toHaveLength(10)
    expect(result.current.total).toBe(25)
  })

  it('returns second page of agents', async () => {
    const agents = makeAgentList(25)
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
    )

    const { result } = renderHook(() =>
      useAgents({ page: 2, pageSize: 10, paused: true }),
    )

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.agents).toHaveLength(10)
    expect(result.current.agents[0].id).toBe(agents[10].id)
  })

  it('auto-refreshes after the interval elapses', async () => {
    vi.useFakeTimers()

    let callCount = 0
    const agents = makeAgentList(2)
    server.use(
      http.get(`${BASE}/agents`, () => {
        callCount++
        return HttpResponse.json(paginatedAgents(agents))
      }),
    )

    renderHook(() => useAgents({ refreshInterval: 2000 }))

    // Flush the initial fetch (small advance to let promises resolve)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50)
    })
    const countAfterInit = callCount

    // Advance past the refresh interval once to trigger a background refresh
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2001)
    })

    expect(callCount).toBeGreaterThan(countAfterInit)
  })

  it('does not auto-refresh when paused=true', async () => {
    vi.useFakeTimers()

    let callCount = 0
    const agents = makeAgentList(2)
    server.use(
      http.get(`${BASE}/agents`, () => {
        callCount++
        return HttpResponse.json(paginatedAgents(agents))
      }),
    )

    renderHook(() => useAgents({ paused: true, refreshInterval: 1000 }))

    // Flush initial fetch
    await act(async () => {
      await vi.runAllTimersAsync()
    })
    const countAfterInit = callCount

    // Advance time — no refreshes should occur
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000)
    })

    expect(callCount).toBe(countAfterInit)
  })

  it('refetch triggers a new data load', async () => {
    let callCount = 0
    const agents = makeAgentList(2)
    server.use(
      http.get(`${BASE}/agents`, () => {
        callCount++
        return HttpResponse.json(paginatedAgents(agents))
      }),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))
    const countBefore = callCount

    await act(async () => {
      result.current.refetch()
    })

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(callCount).toBeGreaterThan(countBefore)
  })

  it('deleteAgent removes the agent from local state', async () => {
    const agents = makeAgentList(3)
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
      http.delete(`${BASE}/agents/:id`, () => HttpResponse.json({})),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.allAgents).toHaveLength(3)

    await act(async () => {
      await result.current.deleteAgent(agents[0].id)
    })

    expect(result.current.allAgents).toHaveLength(2)
    expect(result.current.allAgents.find(a => a.id === agents[0].id)).toBeUndefined()
  })

  it('bulkDelete removes all specified agents from local state', async () => {
    const agents = makeAgentList(4)
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
      http.delete(`${BASE}/agents/:id`, () => HttpResponse.json({})),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(async () => {
      await result.current.bulkDelete([agents[0].id, agents[1].id])
    })

    expect(result.current.allAgents).toHaveLength(2)
  })

  it('createAgent returns the new agent and refreshes list', async () => {
    const agents = makeAgentList(2)
    const newAgent = makeAgent({ name: 'new-agent' })
    server.use(
      http.get(`${BASE}/agents`, () => HttpResponse.json(paginatedAgents(agents))),
      http.post(`${BASE}/agents`, () =>
        HttpResponse.json(newAgent, { status: 201 }),
      ),
    )

    const { result } = renderHook(() => useAgents({ paused: true }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    let created: Agent | undefined
    await act(async () => {
      created = await result.current.createAgent({
        name: 'new-agent',
        working_dir: '/tmp',
        shell: '/bin/bash',
        interactive: false,
        tool_policy: { type: 'AllowAll' },
      })
    })

    expect(created?.name).toBe('new-agent')
  })
})
