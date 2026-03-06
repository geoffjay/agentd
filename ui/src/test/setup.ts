/**
 * Vitest global setup file.
 *
 * Configures:
 *  1. @testing-library/jest-dom matchers
 *  2. MSW server lifecycle (start → reset per test → stop)
 *  3. jest-axe accessibility matchers
 */

import '@testing-library/jest-dom'
import { configureAxe, toHaveNoViolations } from 'jest-axe'
import { expect, beforeAll, afterEach, afterAll, vi } from 'vitest'
import { server } from './mocks/server'

// ---------------------------------------------------------------------------
// WebSocket — jsdom does not implement WebSocket.
// Provide a stub that stays in CONNECTING state so hooks that open WebSocket
// connections don't throw. Individual tests can replace this with a fully
// functional mock (see src/test/mocks/mockWebSocket.ts) when needed.
// ---------------------------------------------------------------------------

class _StubWebSocket {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSING = 2
  static CLOSED = 3

  readonly CONNECTING = 0
  readonly OPEN = 1
  readonly CLOSING = 2
  readonly CLOSED = 3

  readyState = 0 // CONNECTING — never opens, simulates a pending connection
  onopen: ((event: Event) => void) | null = null
  onclose: ((event: CloseEvent) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  constructor(_url: string, _protocols?: string | string[]) {}
  send(_data: string | ArrayBuffer | Blob | ArrayBufferView): void {}
  close(_code?: number, _reason?: string): void {}
  addEventListener(): void {}
  removeEventListener(): void {}
  dispatchEvent(_event: Event): boolean { return true }
}

vi.stubGlobal('WebSocket', _StubWebSocket)

// ---------------------------------------------------------------------------
// window.matchMedia — jsdom does not implement matchMedia, so we provide a
// functional stub that defaults to the light colour scheme. Individual tests
// can override this with vi.fn() if they need to test dark-mode behaviour.
// ---------------------------------------------------------------------------

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: false, // default: light theme / no special media query match
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})

// ---------------------------------------------------------------------------
// Extend Vitest's expect with jest-axe accessibility matchers
// ---------------------------------------------------------------------------

expect.extend(toHaveNoViolations)

// ---------------------------------------------------------------------------
// Configure axe defaults
// ---------------------------------------------------------------------------

export const axe = configureAxe({
  rules: {
    // Disable color-contrast in tests — we focus on structural a11y
    'color-contrast': { enabled: false },
  },
})

// ---------------------------------------------------------------------------
// MSW server lifecycle
// ---------------------------------------------------------------------------

/** Start intercepting requests before all tests in the suite */
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))

/**
 * Reset any runtime handlers added with server.use() so they don't leak
 * into subsequent tests.
 */
afterEach(() => server.resetHandlers())

/** Clean up after all tests are done */
afterAll(() => server.close())
