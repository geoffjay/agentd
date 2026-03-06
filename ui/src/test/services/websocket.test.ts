/**
 * Tests for WebSocketManager.
 *
 * The global WebSocket stub in setup.ts stays in CONNECTING state.
 * These tests install a fully controllable MockWebSocket per test.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { WebSocketManager } from '@/services/websocket'
import { MockWebSocket, installMockWebSocket } from '@/test/mocks/mockWebSocket'
import type { ConnectionState } from '@/services/websocket'

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

describe('WebSocketManager', () => {
  it('starts in Disconnected state', () => {
    const manager = new WebSocketManager('ws://localhost/test')
    expect(manager.state).toBe('Disconnected')
  })

  it('transitions to Connecting on connect()', () => {
    const manager = new WebSocketManager('ws://localhost/test')
    manager.connect()
    expect(manager.state).toBe('Connecting')
  })

  it('transitions to Connected when WebSocket opens', () => {
    const states: ConnectionState[] = []
    const manager = new WebSocketManager('ws://localhost/test')
    manager.onStateChange(s => states.push(s))
    manager.connect()

    lastWs!.simulateOpen()

    expect(manager.state).toBe('Connected')
    expect(states).toContain('Connecting')
    expect(states).toContain('Connected')
  })

  it('transitions to Reconnecting when WebSocket closes unexpectedly', () => {
    vi.useFakeTimers()
    const states: ConnectionState[] = []
    const manager = new WebSocketManager('ws://localhost/test', { minReconnectDelay: 100 })
    manager.onStateChange(s => states.push(s))
    manager.connect()
    lastWs!.simulateOpen()
    lastWs!.simulateClose()

    expect(states).toContain('Reconnecting')
  })

  it('disconnects cleanly and transitions to Disconnected', () => {
    const states: ConnectionState[] = []
    const manager = new WebSocketManager('ws://localhost/test')
    manager.onStateChange(s => states.push(s))
    manager.connect()
    lastWs!.simulateOpen()
    manager.disconnect()

    expect(manager.state).toBe('Disconnected')
    expect(states[states.length - 1]).toBe('Disconnected')
  })

  it('delivers messages to registered handlers', () => {
    const received: string[] = []
    const manager = new WebSocketManager('ws://localhost/test')
    manager.onMessage(event => received.push(String(event.data)))
    manager.connect()
    lastWs!.simulateOpen()
    lastWs!.simulateMessage('hello')
    lastWs!.simulateMessage('world')

    expect(received).toEqual(['hello', 'world'])
  })

  it('removes message handler when cleanup is called', () => {
    const received: string[] = []
    const manager = new WebSocketManager('ws://localhost/test')
    const remove = manager.onMessage(event => received.push(String(event.data)))
    manager.connect()
    lastWs!.simulateOpen()
    lastWs!.simulateMessage('before')
    remove()
    lastWs!.simulateMessage('after')

    expect(received).toEqual(['before'])
  })

  it('buffers messages sent while disconnected and flushes on reconnect', () => {
    const manager = new WebSocketManager('ws://localhost/test')
    manager.connect()
    // Don't open — send while still connecting
    manager.send('buffered-1')
    manager.send('buffered-2')

    lastWs!.simulateOpen()

    expect(lastWs!.sentMessages).toEqual(['buffered-1', 'buffered-2'])
  })

  it('sends messages directly when connected', () => {
    const manager = new WebSocketManager('ws://localhost/test')
    manager.connect()
    lastWs!.simulateOpen()
    manager.send('direct')

    expect(lastWs!.sentMessages).toEqual(['direct'])
  })

  it('does not reconnect after intentional disconnect', () => {
    vi.useFakeTimers()
    let wsCount = 0
    cleanup()
    cleanup = installMockWebSocket(ws => { lastWs = ws; wsCount++ })

    const manager = new WebSocketManager('ws://localhost/test', { minReconnectDelay: 50 })
    manager.connect()
    lastWs!.simulateOpen()
    manager.disconnect()

    vi.advanceTimersByTime(200)

    expect(wsCount).toBe(1) // no reconnect attempts
  })

  it('schedules reconnect with exponential backoff', () => {
    vi.useFakeTimers()
    const wsInstances: MockWebSocket[] = []
    cleanup()
    cleanup = installMockWebSocket(ws => { lastWs = ws; wsInstances.push(ws) })

    const manager = new WebSocketManager('ws://localhost/test', { minReconnectDelay: 100, maxReconnectDelay: 400 })
    manager.connect()
    lastWs!.simulateOpen()
    lastWs!.simulateClose() // triggers first reconnect at 100ms

    vi.advanceTimersByTime(100)
    expect(wsInstances.length).toBe(2)

    lastWs!.simulateClose() // second reconnect at 200ms
    vi.advanceTimersByTime(200)
    expect(wsInstances.length).toBe(3)
  })

  it('sends heartbeat ping when connected and heartbeat is enabled', () => {
    vi.useFakeTimers()
    const manager = new WebSocketManager('ws://localhost/test', { heartbeatInterval: 500 })
    manager.connect()
    lastWs!.simulateOpen()

    vi.advanceTimersByTime(600)

    expect(lastWs!.sentMessages).toContain('ping')
  })

  it('does not send heartbeat when heartbeatInterval is 0', () => {
    vi.useFakeTimers()
    const manager = new WebSocketManager('ws://localhost/test', { heartbeatInterval: 0 })
    manager.connect()
    lastWs!.simulateOpen()

    vi.advanceTimersByTime(30_000)

    expect(lastWs!.sentMessages).not.toContain('ping')
  })

  it('connect() is idempotent when already connecting', () => {
    let wsCount = 0
    cleanup()
    cleanup = installMockWebSocket(() => wsCount++)

    const manager = new WebSocketManager('ws://localhost/test')
    manager.connect()
    manager.connect() // second call should be no-op

    expect(wsCount).toBe(1)
  })
})
