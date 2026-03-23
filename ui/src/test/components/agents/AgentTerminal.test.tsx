/**
 * AgentTerminal tests.
 *
 * xterm.js uses canvas APIs unavailable in jsdom, so the Terminal class and
 * addons are mocked with plain class implementations. The tests cover
 * rendering, toolbar interactions, status badges, and graceful fallback UI.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'

// ---------------------------------------------------------------------------
// Shared mock function references — kept outside vi.mock so tests can inspect
// calls via vi.clearAllMocks() / expect(mockFn).toHaveBeenCalled()
// ---------------------------------------------------------------------------

const mockTermOpen = vi.fn()
const mockTermWrite = vi.fn()
const mockTermWriteln = vi.fn()
const mockTermDispose = vi.fn()
const mockOnDataDispose = vi.fn()
const mockTermOnData = vi.fn(() => ({ dispose: mockOnDataDispose }))
const mockFitAddonFit = vi.fn()
const mockSearchFindNext = vi.fn()
const mockSearchFindPrev = vi.fn()

// ---------------------------------------------------------------------------
// Mock xterm.js — canvas not available in jsdom
// ---------------------------------------------------------------------------

vi.mock('@xterm/xterm', () => {
  class Terminal {
    options: Record<string, unknown> = {}
    cols = 80
    rows = 24
    loadAddon = vi.fn()
    open = mockTermOpen
    write = mockTermWrite
    writeln = mockTermWriteln
    onData = mockTermOnData
    dispose = mockTermDispose
  }
  return { Terminal }
})

vi.mock('@xterm/addon-fit', () => {
  class FitAddon {
    fit = mockFitAddonFit
  }
  return { FitAddon }
})

vi.mock('@xterm/addon-web-links', () => {
  class WebLinksAddon {}
  return { WebLinksAddon }
})

vi.mock('@xterm/addon-search', () => {
  class SearchAddon {
    findNext = mockSearchFindNext
    findPrevious = mockSearchFindPrev
  }
  return { SearchAddon }
})

// xterm.js ships its own stylesheet — suppress the import in test
vi.mock('@xterm/xterm/css/xterm.css', () => ({}))

// ---------------------------------------------------------------------------
// Mock ResizeObserver — not available in jsdom
// ---------------------------------------------------------------------------

class MockResizeObserver {
  observe = vi.fn()
  unobserve = vi.fn()
  disconnect = vi.fn()
}
vi.stubGlobal('ResizeObserver', MockResizeObserver)

// ---------------------------------------------------------------------------
// Import component under test (after mocks are in place)
// ---------------------------------------------------------------------------

import { AgentTerminal } from '@/components/agents/AgentTerminal'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT_ID = 'test-agent-id'

function renderTerminal(props: Partial<Parameters<typeof AgentTerminal>[0]> = {}) {
  return render(<AgentTerminal agentId={AGENT_ID} {...props} />)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AgentTerminal', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the terminal container with aria-label', () => {
    renderTerminal()
    expect(screen.getByLabelText(/agent terminal output/i)).toBeInTheDocument()
  })

  it('shows a "Connecting…" status badge on initial render', () => {
    renderTerminal()
    expect(screen.getByLabelText(/terminal connecting/i)).toBeInTheDocument()
  })

  it('opens the xterm.js terminal on mount', () => {
    renderTerminal()
    expect(mockTermOpen).toHaveBeenCalledOnce()
  })

  it('registers an onData handler for keyboard input', () => {
    renderTerminal()
    expect(mockTermOnData).toHaveBeenCalledOnce()
  })

  it('disposes the terminal on unmount', () => {
    const { unmount } = renderTerminal()
    unmount()
    expect(mockTermDispose).toHaveBeenCalledOnce()
  })

  // ---------------------------------------------------------------------------
  // Toolbar — interactive toggle
  // ---------------------------------------------------------------------------

  describe('interactive mode toggle', () => {
    it('renders a "Read-only" toggle button when readOnly=true (default)', () => {
      renderTerminal({ readOnly: true })
      expect(
        screen.getByRole('button', { name: /switch to interactive mode/i }),
      ).toBeInTheDocument()
      expect(screen.getByText('Read-only')).toBeInTheDocument()
    })

    it('renders an "Interactive" toggle button when readOnly=false', () => {
      renderTerminal({ readOnly: false })
      expect(
        screen.getByRole('button', { name: /switch to read-only mode/i }),
      ).toBeInTheDocument()
      expect(screen.getByText('Interactive')).toBeInTheDocument()
    })

    it('toggles from read-only to interactive on button click', () => {
      renderTerminal({ readOnly: true })
      fireEvent.click(screen.getByRole('button', { name: /switch to interactive mode/i }))
      expect(screen.getByText('Interactive')).toBeInTheDocument()
    })

    it('toggles from interactive to read-only on button click', () => {
      renderTerminal({ readOnly: false })
      fireEvent.click(screen.getByRole('button', { name: /switch to read-only mode/i }))
      expect(screen.getByText('Read-only')).toBeInTheDocument()
    })
  })

  // ---------------------------------------------------------------------------
  // Toolbar — search bar
  // ---------------------------------------------------------------------------

  describe('search bar', () => {
    it('search bar is hidden by default', () => {
      renderTerminal()
      expect(screen.queryByRole('textbox', { name: /search terminal output/i })).toBeNull()
    })

    it('opens search bar when Search button is clicked', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      expect(screen.getByRole('textbox', { name: /search terminal output/i })).toBeInTheDocument()
    })

    it('closes search bar when the × button is clicked', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      fireEvent.click(screen.getByRole('button', { name: /dismiss search/i }))
      expect(screen.queryByRole('textbox', { name: /search terminal output/i })).toBeNull()
    })

    it('closes search bar on Escape key', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      const input = screen.getByRole('textbox', { name: /search terminal output/i })
      fireEvent.keyDown(input, { key: 'Escape' })
      expect(screen.queryByRole('textbox', { name: /search terminal output/i })).toBeNull()
    })

    it('calls findNext on Enter', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      const input = screen.getByRole('textbox', { name: /search terminal output/i })
      fireEvent.change(input, { target: { value: 'hello' } })
      fireEvent.keyDown(input, { key: 'Enter' })
      expect(mockSearchFindNext).toHaveBeenCalledWith('hello', { incremental: false })
    })

    it('calls findPrevious on Shift+Enter', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      const input = screen.getByRole('textbox', { name: /search terminal output/i })
      fireEvent.change(input, { target: { value: 'world' } })
      fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
      expect(mockSearchFindPrev).toHaveBeenCalledWith('world', { incremental: false })
    })

    it('calls findNext when "Next match" button is clicked', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      const input = screen.getByRole('textbox', { name: /search terminal output/i })
      fireEvent.change(input, { target: { value: 'foo' } })
      fireEvent.click(screen.getByRole('button', { name: /next match/i }))
      expect(mockSearchFindNext).toHaveBeenCalledWith('foo', { incremental: false })
    })

    it('calls findPrevious when "Previous match" button is clicked', () => {
      renderTerminal()
      fireEvent.click(screen.getByRole('button', { name: /search terminal output/i }))
      const input = screen.getByRole('textbox', { name: /search terminal output/i })
      fireEvent.change(input, { target: { value: 'bar' } })
      fireEvent.click(screen.getByRole('button', { name: /previous match/i }))
      expect(mockSearchFindPrev).toHaveBeenCalledWith('bar', { incremental: false })
    })
  })

  // ---------------------------------------------------------------------------
  // Unavailable fallback — simulate MAX_CONSECUTIVE_FAILURES close events
  // ---------------------------------------------------------------------------

  describe('unavailable fallback', () => {
    it('shows the fallback after max consecutive connection failures', async () => {
      // Replace the global WebSocket stub with one that fires onclose immediately
      // on construction, simulating a 404 response before the WS handshake.
      const closeCallbacks: Array<() => void> = []

      class FailingWS {
        static CONNECTING = 0
        static OPEN = 1
        static CLOSING = 2
        static CLOSED = 3
        readonly CONNECTING = 0
        readonly OPEN = 1
        readonly CLOSING = 2
        readonly CLOSED = 3
        readyState = 0
        binaryType = 'blob'
        onopen: ((e: Event) => void) | null = null
        onclose: ((e: CloseEvent) => void) | null = null
        onerror: ((e: Event) => void) | null = null
        onmessage: ((e: MessageEvent) => void) | null = null
        constructor() {
          // Queue a close callback so the test can fire it synchronously
          // eslint-disable-next-line @typescript-eslint/no-this-alias
          const self = this
          closeCallbacks.push(() => {
            self.onclose?.(new CloseEvent('close', { code: 1006 }))
          })
        }
        send() {}
        close() {}
        addEventListener() {}
        removeEventListener() {}
        dispatchEvent() {
          return true
        }
      }

      vi.stubGlobal('WebSocket', FailingWS)

      renderTerminal()

      // Fire close events for each connection attempt (MAX_CONSECUTIVE_FAILURES = 3)
      // Each close triggers a setTimeout for reconnect; we fire that callback
      // immediately by also resolving any pending timers.
      vi.useFakeTimers()

      for (let i = 0; i < 3; i++) {
        await act(async () => {
          closeCallbacks[i]?.()
          vi.runAllTimers()
        })
      }

      vi.useRealTimers()

      expect(screen.getByLabelText(/pty not available/i)).toBeInTheDocument()
      expect(screen.getByText(/pty streaming not available/i)).toBeInTheDocument()
      expect(screen.getByRole('button', { name: /retry connection/i })).toBeInTheDocument()
    })
  })
})
