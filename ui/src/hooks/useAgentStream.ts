/**
 * useAgentStream — WebSocket hook for real-time agent log streaming.
 *
 * Features:
 * - Connects to ws://<host>/ws/<agentId> on mount
 * - Auto-reconnect on disconnect with exponential backoff
 * - Maintains a capped circular buffer of the last MAX_LINES log lines
 * - Exposes stream status: 'connecting' | 'connected' | 'disconnected'
 * - clear() resets the visible buffer without affecting the stream
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type StreamStatus = 'connecting' | 'connected' | 'disconnected'

export interface LogLine {
  id: number
  /** Raw text (may contain ANSI escape sequences) */
  text: string
  timestamp: string
}

export interface UseAgentStreamResult {
  lines: LogLine[]
  status: StreamStatus
  /** Clear all buffered log lines (does not disconnect the stream) */
  clear: () => void
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_LINES = 5000
const MIN_RECONNECT_DELAY_MS = 1_000
const MAX_RECONNECT_DELAY_MS = 30_000

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let globalLineId = 0

function makeLogLine(text: string): LogLine {
  return {
    id: ++globalLineId,
    text,
    timestamp: new Date().toISOString(),
  }
}

/** Cap the buffer at MAX_LINES, discarding oldest entries */
function capLines(prev: LogLine[], incoming: LogLine[]): LogLine[] {
  const combined = [...prev, ...incoming]
  if (combined.length <= MAX_LINES) return combined
  return combined.slice(combined.length - MAX_LINES)
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgentStream(agentId: string): UseAgentStreamResult {
  const [lines, setLines] = useState<LogLine[]>([])
  const [status, setStatus] = useState<StreamStatus>('connecting')

  const wsRef = useRef<WebSocket | null>(null)
  const reconnectDelayRef = useRef(MIN_RECONNECT_DELAY_MS)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const unmountedRef = useRef(false)

  const connect = useCallback(() => {
    if (unmountedRef.current) return

    setStatus('connecting')

    const ws = orchestratorClient.connectAgentStream(agentId)
    wsRef.current = ws

    ws.onopen = () => {
      if (unmountedRef.current) { ws.close(); return }
      setStatus('connected')
      reconnectDelayRef.current = MIN_RECONNECT_DELAY_MS
    }

    ws.onmessage = (event: MessageEvent) => {
      if (unmountedRef.current) return
      // Each message may be a single line or newline-separated block
      const rawText = String(event.data)
      const incoming = rawText
        .split('\n')
        .filter(Boolean)
        .map(makeLogLine)
      setLines(prev => capLines(prev, incoming))
    }

    ws.onerror = () => {
      // onerror fires before onclose; the actual reconnect is scheduled in onclose
    }

    ws.onclose = () => {
      if (unmountedRef.current) return
      setStatus('disconnected')
      wsRef.current = null

      // Exponential backoff reconnect
      const delay = reconnectDelayRef.current
      reconnectDelayRef.current = Math.min(delay * 2, MAX_RECONNECT_DELAY_MS)

      reconnectTimerRef.current = setTimeout(() => {
        if (!unmountedRef.current) connect()
      }, delay)
    }
  }, [agentId])

  useEffect(() => {
    unmountedRef.current = false
    connect()

    return () => {
      unmountedRef.current = true
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current)
      }
      if (wsRef.current) {
        wsRef.current.onclose = null // prevent reconnect on intentional close
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, [connect])

  const clear = useCallback(() => setLines([]), [])

  return { lines, status, clear }
}
