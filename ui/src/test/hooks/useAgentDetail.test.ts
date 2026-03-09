/**
 * Tests for useAgentDetail hook.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { useAgentDetail } from '@/hooks/useAgentDetail'
import { server } from '@/test/mocks/server'
import { makeAgent, makePendingApproval, resetAgentSeq } from '@/test/mocks/factories'
import type { PaginatedResponse } from '@/types/common'
import type { PendingApproval } from '@/types/orchestrator'

const BASE = 'http://localhost:17006'

function paginatedApprovals(items: PendingApproval[]): PaginatedResponse<PendingApproval> {
  return { items, total: items.length, limit: 50, offset: 0 }
}

beforeEach(() => resetAgentSeq())
afterEach(() => vi.useRealTimers())

describe('useAgentDetail', () => {
  it('fetches agent on mount', async () => {
    const agent = makeAgent({ id: 'agent-1' })
    server.use(
      http.get(`${BASE}/agents/agent-1`, () => HttpResponse.json(agent)),
      http.get(`${BASE}/agents/agent-1/approvals`, () => HttpResponse.json(paginatedApprovals([]))),
    )

    const { result } = renderHook(() => useAgentDetail('agent-1', { refreshInterval: 0 }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.agent?.id).toBe('agent-1')
    expect(result.current.agent?.name).toBe(agent.name)
  })

  it('sets error when agent fetch fails', async () => {
    server.use(
      http.get(`${BASE}/agents/bad-id`, () => HttpResponse.error()),
      http.get(`${BASE}/agents/bad-id/approvals`, () => HttpResponse.json(paginatedApprovals([]))),
    )

    const { result } = renderHook(() => useAgentDetail('bad-id', { refreshInterval: 0 }))

    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBeDefined()
    expect(result.current.agent).toBeNull()
  })

  it('fetches pending approvals on mount', async () => {
    const agent = makeAgent({ id: 'agent-2' })
    const approvals = [makePendingApproval({ agent_id: 'agent-2' })]
    server.use(
      http.get(`${BASE}/agents/agent-2`, () => HttpResponse.json(agent)),
      http.get(`${BASE}/agents/agent-2/approvals`, () =>
        HttpResponse.json(paginatedApprovals(approvals)),
      ),
    )

    const { result } = renderHook(() => useAgentDetail('agent-2', { refreshInterval: 0 }))

    await waitFor(() => expect(result.current.approvalsLoading).toBe(false))

    expect(result.current.approvals).toHaveLength(1)
    expect(result.current.approvals[0].tool_name).toBe(approvals[0].tool_name)
  })

  it('updateModel updates local agent state', async () => {
    const agent = makeAgent({ id: 'agent-3' })
    const updatedAgent = { ...agent, config: { ...agent.config, model: 'new-model' } }
    server.use(
      http.get(`${BASE}/agents/agent-3`, () => HttpResponse.json(agent)),
      http.get(`${BASE}/agents/agent-3/approvals`, () => HttpResponse.json(paginatedApprovals([]))),
      http.put(`${BASE}/agents/agent-3/model`, () => HttpResponse.json(updatedAgent)),
    )

    const { result } = renderHook(() => useAgentDetail('agent-3', { refreshInterval: 0 }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    await act(async () => {
      await result.current.updateModel({ model: 'new-model', restart: false })
    })

    expect(result.current.agent?.config.model).toBe('new-model')
  })

  it('approveRequest removes approval from local state', async () => {
    const agent = makeAgent({ id: 'agent-4' })
    const approval = makePendingApproval({ agent_id: 'agent-4' })
    server.use(
      http.get(`${BASE}/agents/agent-4`, () => HttpResponse.json(agent)),
      http.get(`${BASE}/agents/agent-4/approvals`, () =>
        HttpResponse.json(paginatedApprovals([approval])),
      ),
      http.post(`${BASE}/approvals/${approval.id}/approve`, () =>
        HttpResponse.json({ ...approval, status: 'Approved' }),
      ),
    )

    const { result } = renderHook(() => useAgentDetail('agent-4', { refreshInterval: 0 }))
    await waitFor(() => expect(result.current.approvalsLoading).toBe(false))
    expect(result.current.approvals).toHaveLength(1)

    await act(async () => {
      await result.current.approveRequest(approval.id)
    })

    expect(result.current.approvals).toHaveLength(0)
  })

  it('denyRequest removes approval from local state', async () => {
    const agent = makeAgent({ id: 'agent-5' })
    const approval = makePendingApproval({ agent_id: 'agent-5' })
    server.use(
      http.get(`${BASE}/agents/agent-5`, () => HttpResponse.json(agent)),
      http.get(`${BASE}/agents/agent-5/approvals`, () =>
        HttpResponse.json(paginatedApprovals([approval])),
      ),
      http.post(`${BASE}/approvals/${approval.id}/deny`, () =>
        HttpResponse.json({ ...approval, status: 'Denied' }),
      ),
    )

    const { result } = renderHook(() => useAgentDetail('agent-5', { refreshInterval: 0 }))
    await waitFor(() => expect(result.current.approvalsLoading).toBe(false))

    await act(async () => {
      await result.current.denyRequest(approval.id)
    })

    expect(result.current.approvals).toHaveLength(0)
  })

  it('auto-refreshes at the configured interval', async () => {
    vi.useFakeTimers()

    let callCount = 0
    const agent = makeAgent({ id: 'agent-6' })
    server.use(
      http.get(`${BASE}/agents/agent-6`, () => {
        callCount++
        return HttpResponse.json(agent)
      }),
      http.get(`${BASE}/agents/agent-6/approvals`, () => HttpResponse.json(paginatedApprovals([]))),
    )

    renderHook(() => useAgentDetail('agent-6', { refreshInterval: 2000 }))

    await act(async () => {
      await vi.advanceTimersByTimeAsync(50)
    })
    const countAfterMount = callCount

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2001)
    })

    expect(callCount).toBeGreaterThan(countAfterMount)
  })
})
