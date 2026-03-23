/**
 * useAgentTerminal — xterm.js + WebSocket lifecycle hook for PTY streaming.
 *
 * Connects to ws://{orchestrator}/terminal/{agentId} when the terminal tab
 * is first activated. Feeds binary PTY output into an xterm.js Terminal
 * instance and handles resize events via FitAddon.
 *
 * Connection states:
 *   connecting    → WS opened, waiting for first data
 *   connected     → receiving PTY output
 *   reconnecting  → WS dropped, WebSocketManager backing off
 *   disconnected  → intentional close or agent terminated
 *   unavailable   → backend returned 404 or {"error":"pty_not_supported"}
 *
 * Lazy connection: the WS and terminal are created only when `activate()`
 * is called for the first time. Subsequent tab switches do not reconnect.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import { serviceConfig } from '@/services/config'
import { XTERM_THEME } from '@/styles/themes'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type TerminalStatus =
  | 'idle'          // not yet activated
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'disconnected'
  | 'unavailable'   // PTY backend not supported

export interface UseAgentTerminalResult {
  /** Current connection status */
  status: TerminalStatus
  /** Ref to attach to the xterm container div */
  containerRef: React.RefObject<HTMLDivElement | null>
  /** Whether keyboard input is forwarded to PTY */
  interactive: boolean
  /** Toggle interactive mode */
  setInteractive: (value: boolean) => void
  /** Activate the terminal (call when Terminal tab first becomes visible) */
  activate: () => void
}

// ---------------------------------------------------------------------------
// WebSocket URL helper
// ---------------------------------------------------------------------------

function terminalWsUrl(agentId: string): string {
  const base = serviceConfig.orchestratorServiceUrl
  const ws = base.replace(/^http/, 'ws')
  return `${ws}/terminal/${agentId}`
}

// ---------------------------------------------------------------------------
// Resize debounce
// ---------------------------------------------------------------------------

function debounce<T extends (...args: unknown[]) => void>(fn: T, ms: number): T {
  let timer: ReturnType<typeof setTimeout> | null = null
  return ((...args: unknown[]) => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = null
      fn(...args)
    }, ms)
  }) as T
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgentTerminal(agentId: string): UseAgentTerminalResult {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [status, setStatus] = useState<TerminalStatus>('idle')
  const [interactive, setInteractiveState] = useState(false)

  // Stable refs for xterm and WS instances — never triggers re-renders
  const terminalRef = useRef<import('@xterm/xterm').Terminal | null>(null)
  const fitAddonRef = useRef<import('@xterm/addon-fit').FitAddon | null>(null)
  const wsRef = useRef<WebSocketManager | null>(null)
  const activatedRef = useRef(false)
  const interactiveRef = useRef(false)
  const decoderRef = useRef(new TextDecoder())

  // Keep interactiveRef in sync for use inside WS message handler
  const setInteractive = useCallback((value: boolean) => {
    interactiveRef.current = value
    setInteractiveState(value)
  }, [])

  // ---------------------------------------------------------------------------
  // Initialise xterm (called once on first activation)
  // ---------------------------------------------------------------------------

  const initTerminal = useCallback(async () => {
    if (!containerRef.current) return

    // Dynamic import — keeps @xterm out of the initial bundle
    const [{ Terminal }, { FitAddon }] = await Promise.all([
      import('@xterm/xterm'),
      import('@xterm/addon-fit'),
    ])

    // Respect reduced-motion preference for cursor blink
    const prefersReducedMotion = window.matchMedia(
      '(prefers-reduced-motion: reduce)',
    ).matches

    const terminal = new Terminal({
      theme: XTERM_THEME,
      fontFamily: '"JetBrains Mono", "Fira Code", Consolas, "Courier New", monospace',
      fontSize: 12,
      lineHeight: 1.4,
      cursorBlink: !prefersReducedMotion,
      scrollback: 5000,
      // Disable xterm's built-in right-click context menu — let browser handle it
      rightClickSelectsWord: true,
      // Allow native copy on selection (Ctrl+Shift+C / Cmd+C)
      copyOnSelection: false,
    })

    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.open(containerRef.current)
    fitAddon.fit()

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    // Forward keyboard input when in interactive mode
    terminal.onData((data) => {
      if (interactiveRef.current && wsRef.current) {
        wsRef.current.send(JSON.stringify({ type: 'input', data }))
      }
    })
  }, [])

  // ---------------------------------------------------------------------------
  // Resize handling via ResizeObserver + FitAddon
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!containerRef.current) return

    const sendResize = debounce(() => {
      const fitAddon = fitAddonRef.current
      const ws = wsRef.current
      if (!fitAddon || !terminalRef.current) return
      fitAddon.fit()
      const { cols, rows } = terminalRef.current
      if (ws) {
        ws.send(JSON.stringify({ type: 'resize', cols, rows }))
      }
    }, 100)

    const observer = new ResizeObserver(sendResize)
    observer.observe(containerRef.current)
    return () => observer.disconnect()
  }, [])

  // ---------------------------------------------------------------------------
  // Activate (called when Terminal tab first becomes visible)
  // ---------------------------------------------------------------------------

  const activate = useCallback(async () => {
    if (activatedRef.current) return
    activatedRef.current = true

    await initTerminal()

    const url = terminalWsUrl(agentId)
    const ws = new WebSocketManager(url)
    wsRef.current = ws

    ws.onStateChange((state) => {
      switch (state) {
        case 'Connecting':
          setStatus('connecting')
          break
        case 'Connected':
          setStatus('connected')
          break
        case 'Reconnecting': {
          setStatus('reconnecting')
          // Inject a visual separator into the terminal
          terminalRef.current?.write(
            '\r\n\x1b[2m\x1b[33m─── reconnecting… ───\x1b[0m\r\n',
          )
          break
        }
        case 'Disconnected':
          setStatus('disconnected')
          break
      }
    })

    ws.onMessage((event) => {
      const terminal = terminalRef.current
      if (!terminal) return

      // Binary frame — raw PTY output
      if (event.data instanceof ArrayBuffer) {
        terminal.write(decoderRef.current.decode(event.data))
        return
      }

      // Text frame — JSON control message or raw string
      if (typeof event.data === 'string') {
        // Try to parse as JSON control message first
        try {
          const msg = JSON.parse(event.data) as Record<string, unknown>
          if (msg.error === 'pty_not_supported') {
            setStatus('unavailable')
            ws.disconnect()
            return
          }
          // Unknown JSON — ignore silently
          return
        } catch {
          // Not JSON — treat as raw terminal text
          terminal.write(event.data)
        }
      }
    })

    ws.connect()
  }, [agentId, initTerminal])

  // ---------------------------------------------------------------------------
  // Handle 404 (endpoint not found → unavailable)
  // We detect this via the WS close code 1006 immediately after connect,
  // or via the HTTP upgrade failing. The simplest heuristic: if we transition
  // Connecting → Disconnected within 500ms without receiving any data,
  // treat as unavailable.
  // ---------------------------------------------------------------------------

  const firstDataRef = useRef(false)
  const connectTimeRef = useRef<number | null>(null)

  useEffect(() => {
    if (status === 'connecting') {
      connectTimeRef.current = Date.now()
      firstDataRef.current = false
    }
    if (status === 'disconnected' && connectTimeRef.current !== null) {
      const elapsed = Date.now() - connectTimeRef.current
      if (!firstDataRef.current && elapsed < 800) {
        setStatus('unavailable')
      }
    }
  }, [status])

  // ---------------------------------------------------------------------------
  // Cleanup on unmount
  // ---------------------------------------------------------------------------

  useEffect(() => {
    return () => {
      wsRef.current?.disconnect()
      terminalRef.current?.dispose()
    }
  }, [])

  return {
    status,
    containerRef,
    interactive,
    setInteractive,
    activate,
  }
}
