/**
 * Tests for useAgentUsage hook — real-time event bus integration.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useAgentUsage } from '@/hooks/useAgentUsage'
import { agentEventBus } from '@/services/eventBus'
import type { UsageUpdateEvent, ContextClearedEvent } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Mock orchestratorClient
// ---------------------------------------------------------------------------

vi.mock('@/services/orchestrator', () => ({
  orchestratorClient: {
    getAgentUsage: vi.fn().mockResolvedValue({
      agent_id: 'agent-1',
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
    clearContext: vi.fn().mockResolvedValue({
      agent_id: 'agent-1',
      new_session_number: 2,
    }),
  },
}))

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useAgentUsage real-time events', () => {
  it('loads initial usage data from API', async () => {
    vi.useRealTimers()
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    expect(result.current.usage?.cumulative.input_tokens).toBe(200)
    expect(result.current.loading).toBe(false)
  })

  it('optimistically updates usage on agent:usage_update event', async () => {
    vi.useRealTimers()
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    const prevInput = result.current.usage!.cumulative.input_tokens

    act(() => {
      agentEventBus.emit<UsageUpdateEvent>({
        type: 'agent:usage_update',
        agentId: 'agent-1',
        session_number: 1,
        usage: {
          input_tokens: 50,
          output_tokens: 25,
          cache_read_input_tokens: 10,
          cache_creation_input_tokens: 5,
          total_cost_usd: 0.005,
          num_turns: 1,
          duration_ms: 500,
          duration_api_ms: 400,
        },
        timestamp: new Date().toISOString(),
      })
    })

    expect(result.current.usage!.cumulative.input_tokens).toBe(prevInput + 50)
    expect(result.current.usage!.cumulative.output_tokens).toBe(125)
    expect(result.current.usage!.cumulative.result_count).toBe(5)
  })

  it('ignores usage_update events for other agents', async () => {
    vi.useRealTimers()
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    const prevInput = result.current.usage!.cumulative.input_tokens

    act(() => {
      agentEventBus.emit<UsageUpdateEvent>({
        type: 'agent:usage_update',
        agentId: 'agent-OTHER',
        session_number: 1,
        usage: {
          input_tokens: 999,
          output_tokens: 0,
          cache_read_input_tokens: 0,
          cache_creation_input_tokens: 0,
          total_cost_usd: 0,
          num_turns: 1,
          duration_ms: 0,
          duration_api_ms: 0,
        },
        timestamp: new Date().toISOString(),
      })
    })

    expect(result.current.usage!.cumulative.input_tokens).toBe(prevInput)
  })

  it('resets current session on context_cleared event', async () => {
    vi.useRealTimers()
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    const prevSessionCount = result.current.usage!.session_count

    act(() => {
      agentEventBus.emit<ContextClearedEvent>({
        type: 'agent:context_cleared',
        agentId: 'agent-1',
        new_session_number: 2,
        timestamp: '2024-06-01T12:00:00Z',
      })
    })

    expect(result.current.usage!.current_session?.input_tokens).toBe(0)
    expect(result.current.usage!.current_session?.output_tokens).toBe(0)
    expect(result.current.usage!.current_session?.started_at).toBe('2024-06-01T12:00:00Z')
    expect(result.current.usage!.session_count).toBe(prevSessionCount + 1)
    // Cumulative should be unchanged
    expect(result.current.usage!.cumulative.input_tokens).toBe(200)
  })

  it('ignores context_cleared events for other agents', async () => {
    vi.useRealTimers()
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    const prevSessionCount = result.current.usage!.session_count

    act(() => {
      agentEventBus.emit<ContextClearedEvent>({
        type: 'agent:context_cleared',
        agentId: 'agent-OTHER',
        new_session_number: 5,
        timestamp: new Date().toISOString(),
      })
    })

    expect(result.current.usage!.session_count).toBe(prevSessionCount)
  })
})
