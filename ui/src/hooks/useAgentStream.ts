/**
 * useAgentStream — WebSocket hook for real-time agent log streaming.
 *
 * Connects to ws://<host>/ws/<agentId> via WebSocketManager, which
 * handles auto-reconnect (exponential backoff), heartbeat detection,
 * and message buffering.
 *
 * Returns a capped circular buffer of up to MAX_LINES log lines plus
 * a connection status indicator.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import { serviceConfig } from '@/services/config'

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

const MAX_LINES = 5_000

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

function capLines(prev: LogLine[], incoming: LogLine[]): LogLine[] {
  const combined = [...prev, ...incoming]
  if (combined.length <= MAX_LINES) return combined
  return combined.slice(combined.length - MAX_LINES)
}

function agentStreamUrl(agentId: string): string {
  const wsBase = serviceConfig.orchestratorServiceUrl.replace(/^http/, 'ws')
  return `${wsBase}/ws/${agentId}`
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgentStream(agentId: string): UseAgentStreamResult {
  const [lines, setLines] = useState<LogLine[]>([])
  const [status, setStatus] = useState<StreamStatus>('connecting')

  const managerRef = useRef<WebSocketManager | null>(null)

  useEffect(() => {
    const manager = new WebSocketManager(agentStreamUrl(agentId), {
      // Disable heartbeat — agent output arrives irregularly; a ping
      // would be noise in the log stream
      heartbeatInterval: 0,
    })
    managerRef.current = manager

    const unsubState = manager.onStateChange((state) => {
      switch (state) {
        case 'Connected':
          setStatus('connected')
          break
        case 'Disconnected':
          setStatus('disconnected')
          break
        default:
          // Connecting | Reconnecting → show as 'connecting'
          setStatus('connecting')
      }
    })

    const unsubMsg = manager.onMessage((event: MessageEvent) => {
      const rawText = String(event.data)
      const incoming = rawText.split('\n').filter(Boolean).map(makeLogLine)
      setLines((prev) => capLines(prev, incoming))
    })

    manager.connect()

    return () => {
      unsubState()
      unsubMsg()
      manager.disconnect()
      managerRef.current = null
    }
  }, [agentId])

  const clear = useCallback(() => setLines([]), [])

  return { lines, status, clear }
}
