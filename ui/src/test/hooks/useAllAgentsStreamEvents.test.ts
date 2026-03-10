/**
 * Tests for useAllAgentsStream — usage_update and context_cleared event handling.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useAllAgentsStream } from '@/hooks/useAllAgentsStream'
import { MockWebSocket, installMockWebSocket } from '@/test/mocks/mockWebSocket'
import { agentEventBus } from '@/services/eventBus'
import type { UsageUpdateEvent, ContextClearedEvent } from '@/types/orchestrator'

let lastWs: MockWebSocket | undefined
let cleanup: () => void

beforeEach(() => {
  lastWs = undefined
  cleanup = installMockWebSocket((ws) => {
    lastWs = ws
  })
})

afterEach(() => {
  vi.useRealTimers()
  cleanup()
})

describe('useAllAgentsStream usage events', () => {
  it('tracks usage_update events in usageUpdates map', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:usage_update',
          agentId: 'agent-abc',
          session_number: 1,
          usage: {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 5,
            total_cost_usd: 0.01,
            num_turns: 1,
            duration_ms: 500,
            duration_api_ms: 400,
          },
          timestamp: '2024-01-01T00:00:00Z',
        }),
      )
    })

    expect(result.current.usageUpdates.get('agent-abc')).toBeDefined()
    expect(result.current.usageUpdates.get('agent-abc')!.usage.input_tokens).toBe(100)
  })

  it('tracks context_cleared events in contextClears map', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:context_cleared',
          agentId: 'agent-xyz',
          new_session_number: 3,
          timestamp: '2024-01-01T12:00:00Z',
        }),
      )
    })

    expect(result.current.contextClears.get('agent-xyz')).toBeDefined()
    expect(result.current.contextClears.get('agent-xyz')!.new_session_number).toBe(3)
  })

  it('emits usage_update events to the agentEventBus', async () => {
    const received: UsageUpdateEvent[] = []
    const unsub = agentEventBus.on<UsageUpdateEvent>('agent:usage_update', (event) => {
      received.push(event)
    })

    renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:usage_update',
          agentId: 'agent-bus',
          session_number: 1,
          usage: {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            total_cost_usd: 0.001,
            num_turns: 1,
            duration_ms: 100,
            duration_api_ms: 80,
          },
          timestamp: new Date().toISOString(),
        }),
      )
    })

    expect(received).toHaveLength(1)
    expect(received[0].agentId).toBe('agent-bus')
    unsub()
  })

  it('emits context_cleared events to the agentEventBus', async () => {
    const received: ContextClearedEvent[] = []
    const unsub = agentEventBus.on<ContextClearedEvent>('agent:context_cleared', (event) => {
      received.push(event)
    })

    renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:context_cleared',
          agentId: 'agent-cleared',
          new_session_number: 2,
          timestamp: new Date().toISOString(),
        }),
      )
    })

    expect(received).toHaveLength(1)
    expect(received[0].agentId).toBe('agent-cleared')
    unsub()
  })

  it('replaces previous usage_update for same agent with latest', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    const makeUsageEvent = (tokens: number) =>
      JSON.stringify({
        type: 'agent:usage_update',
        agentId: 'agent-same',
        session_number: 1,
        usage: {
          input_tokens: tokens,
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

    await act(async () => {
      lastWs!.simulateMessage(makeUsageEvent(100))
      lastWs!.simulateMessage(makeUsageEvent(200))
    })

    // Should have the latest value
    expect(result.current.usageUpdates.get('agent-same')!.usage.input_tokens).toBe(200)
  })
})
