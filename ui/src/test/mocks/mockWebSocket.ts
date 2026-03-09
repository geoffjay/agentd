/**
 * MockWebSocket — a controllable WebSocket replacement for unit tests.
 *
 * Usage:
 *   import { MockWebSocket, installMockWebSocket } from '@/test/mocks/mockWebSocket'
 *
 *   let lastSocket: MockWebSocket
 *   installMockWebSocket(ws => { lastSocket = ws })
 *
 *   // ... render component / hook ...
 *   lastSocket.simulateOpen()
 *   lastSocket.simulateMessage('hello')
 *   lastSocket.simulateClose()
 */

import { vi } from 'vitest'

export type MockWebSocketFactory = (ws: MockWebSocket) => void

export class MockWebSocket {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSING = 2
  static CLOSED = 3

  readonly CONNECTING = 0
  readonly OPEN = 1
  readonly CLOSING = 2
  readonly CLOSED = 3

  readyState: number = MockWebSocket.CONNECTING

  onopen: ((event: Event) => void) | null = null
  onclose: ((event: CloseEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null

  sentMessages: string[] = []
  readonly url: string

  constructor(url: string) {
    this.url = url
  }

  send(data: string): void {
    this.sentMessages.push(data)
  }

  close(code = 1000, reason = ''): void {
    if (this.readyState === MockWebSocket.CLOSED) return
    this.readyState = MockWebSocket.CLOSED
    if (this.onclose) {
      this.onclose(new CloseEvent('close', { code, reason, wasClean: code === 1000 }))
    }
  }

  addEventListener(): void {}
  removeEventListener(): void {}
  dispatchEvent(_event: Event): boolean {
    return true
  }

  // -------------------------------------------------------------------------
  // Test helpers — call these from test code to simulate server behaviour
  // -------------------------------------------------------------------------

  /** Simulate the WebSocket connection being established */
  simulateOpen(): void {
    this.readyState = MockWebSocket.OPEN
    if (this.onopen) {
      this.onopen(new Event('open'))
    }
  }

  /** Simulate the server sending a text message */
  simulateMessage(data: string): void {
    if (this.onmessage) {
      this.onmessage(new MessageEvent('message', { data }))
    }
  }

  /** Simulate an error event (followed by close) */
  simulateError(): void {
    if (this.onerror) {
      this.onerror(new Event('error'))
    }
    this.simulateClose(1006, 'Abnormal closure')
  }

  /** Simulate the connection being closed by the server */
  simulateClose(code = 1006, reason = ''): void {
    this.readyState = MockWebSocket.CLOSED
    if (this.onclose) {
      this.onclose(new CloseEvent('close', { code, reason, wasClean: false }))
    }
  }
}

// ---------------------------------------------------------------------------
// Installation helper
// ---------------------------------------------------------------------------

/**
 * Replace the global WebSocket with MockWebSocket and call `onCreate`
 * each time a new socket is constructed. Returns a cleanup function that
 * restores the original WebSocket.
 */
export function installMockWebSocket(onCreate?: MockWebSocketFactory): () => void {
  const instances: MockWebSocket[] = []

  function MockWebSocketConstructor(url: string) {
    const ws = new MockWebSocket(url)
    instances.push(ws)
    onCreate?.(ws)
    return ws
  }

  MockWebSocketConstructor.CONNECTING = 0
  MockWebSocketConstructor.OPEN = 1
  MockWebSocketConstructor.CLOSING = 2
  MockWebSocketConstructor.CLOSED = 3

  vi.stubGlobal('WebSocket', MockWebSocketConstructor)

  return () => {
    vi.unstubAllGlobals()
  }
}

/** Returns all MockWebSocket instances created since installMockWebSocket was called */
export function getLastMockWebSocket(instances: MockWebSocket[]): MockWebSocket | undefined {
  return instances[instances.length - 1]
}
