/**
 * Tests for useWebSocket hook.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useWebSocket } from '@/hooks/useWebSocket'
import { MockWebSocket, installMockWebSocket } from '@/test/mocks/mockWebSocket'

let lastWs: MockWebSocket | undefined
let cleanup: () => void

beforeEach(() => {
  lastWs = undefined
  cleanup = installMockWebSocket(ws => { lastWs = ws })
})

afterEach(() => {
  vi.useRealTimers()
  cleanup()
})

describe('useWebSocket', () => {
  it('starts in Disconnected state', () => {
    const { result } = renderHook(() =>
      useWebSocket('ws://localhost/test', { paused: true }),
    )
    expect(result.current.connectionState).toBe('Disconnected')
  })

  it('transitions to Connecting on mount when not paused', async () => {
    const { result } = renderHook(() =>
      useWebSocket('ws://localhost/test'),
    )
    await waitFor(() => expect(result.current.connectionState).toBe('Connecting'))
  })

  it('transitions to Connected when WebSocket opens', async () => {
    const { result } = renderHook(() => useWebSocket('ws://localhost/test'))

    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
    })

    expect(result.current.connectionState).toBe('Connected')
  })

  it('receives messages and accumulates them', async () => {
    const { result } = renderHook(() => useWebSocket('ws://localhost/test'))
    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
      lastWs!.simulateMessage('msg-1')
      lastWs!.simulateMessage('msg-2')
    })

    expect(result.current.messages).toHaveLength(2)
    expect(result.current.messages[0].data).toBe('msg-1')
    expect(result.current.messages[1].data).toBe('msg-2')
  })

  it('caps messages at maxMessages', async () => {
    const { result } = renderHook(() =>
      useWebSocket('ws://localhost/test', { maxMessages: 3 }),
    )
    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
      for (let i = 0; i < 5; i++) {
        lastWs!.simulateMessage(`msg-${i}`)
      }
    })

    expect(result.current.messages).toHaveLength(3)
    expect(result.current.messages[0].data).toBe('msg-2')
  })

  it('send() transmits when connected', async () => {
    const { result } = renderHook(() => useWebSocket('ws://localhost/test'))
    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
    })

    act(() => {
      result.current.send('hello')
    })

    expect(lastWs!.sentMessages).toContain('hello')
  })

  it('disconnect() closes the connection', async () => {
    const { result } = renderHook(() => useWebSocket('ws://localhost/test'))
    await waitFor(() => expect(lastWs).toBeDefined())

    await act(async () => {
      lastWs!.simulateOpen()
    })

    await act(async () => {
      result.current.disconnect()
    })

    expect(result.current.connectionState).toBe('Disconnected')
  })

  it('does not auto-connect when paused=true', async () => {
    let wsCreated = false
    cleanup()
    cleanup = installMockWebSocket(() => { wsCreated = true })

    renderHook(() => useWebSocket('ws://localhost/test', { paused: true }))

    // Give React time to run effects
    await new Promise(r => setTimeout(r, 10))
    expect(wsCreated).toBe(false)
  })
})
