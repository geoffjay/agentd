/**
 * Tests for useAllAgentsStream hook.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useAllAgentsStream } from '@/hooks/useAllAgentsStream'
import { MockWebSocket, installMockWebSocket } from '@/test/mocks/mockWebSocket'
import { agentEventBus } from '@/services/eventBus'

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

describe('useAllAgentsStream', () => {
  it('starts connecting on mount (not yet open)', () => {
    // useAllAgentsStream connects immediately on mount so by the time
    // renderHook returns (after flushing effects in act) the state is
    // 'Connecting' — the mock socket hasn't opened yet.
    const { result } = renderHook(() => useAllAgentsStream())
    expect(result.current.connectionState).toBe('Connecting')
    expect(lastWs).toBeDefined()
  })

  it('becomes Connected after WebSocket opens', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
    })

    expect(result.current.connectionState).toBe('Connected')
  })

  it('parses agent:output events and adds lines to agentMessages', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:output',
          agentId: 'agent-abc',
          line: 'hello from agent',
          timestamp: new Date().toISOString(),
        }),
      )
    })

    expect(result.current.agentMessages.get('agent-abc')).toHaveLength(1)
    expect(result.current.agentMessages.get('agent-abc')![0].text).toBe('hello from agent')
  })

  it('demultiplexes messages to different agents', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:output',
          agentId: 'agent-1',
          line: 'line-a',
          timestamp: new Date().toISOString(),
        }),
      )
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:output',
          agentId: 'agent-2',
          line: 'line-b',
          timestamp: new Date().toISOString(),
        }),
      )
    })

    expect(result.current.agentMessages.get('agent-1')).toHaveLength(1)
    expect(result.current.agentMessages.get('agent-2')).toHaveLength(1)
  })

  it('emits events to the agentEventBus', async () => {
    const received: string[] = []
    const unsub = agentEventBus.on('agent:status_change', (event) => {
      received.push(event.agentId)
    })

    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage(
        JSON.stringify({
          type: 'agent:status_change',
          agentId: 'agent-xyz',
          status: 'Running',
          timestamp: new Date().toISOString(),
        }),
      )
    })

    expect(received).toContain('agent-xyz')
    unsub()
    result.current // suppress unused warning
  })

  it('handles non-JSON messages gracefully (stored under "unknown")', async () => {
    const { result } = renderHook(() => useAllAgentsStream())
    await waitFor(() => expect(lastWs).toBeDefined())
    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      lastWs!.simulateMessage('plain text line')
    })

    expect(result.current.agentMessages.get('unknown')).toHaveLength(1)
    expect(result.current.agentMessages.get('unknown')![0].text).toBe('plain text line')
  })
})
