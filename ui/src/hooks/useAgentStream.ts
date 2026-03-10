/**
 * useAgentStream — WebSocket hook for real-time agent log streaming.
 *
 * Connects to ws://<host>/stream/<agentId> via WebSocketManager, which
 * handles auto-reconnect (exponential backoff), heartbeat detection,
 * and message buffering.
 *
 * Returns a capped circular buffer of up to MAX_LINES log lines plus
 * a connection status indicator.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import { serviceConfig } from '@/services/config'
import { agentEventBus } from '@/services/eventBus'
import type { AgentEvent, UsageUpdateEvent, ContextClearedEvent } from '@/types/orchestrator'

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

/** Callback invoked when a real-time usage update event arrives */
export type UsageUpdateCallback = (event: UsageUpdateEvent) => void

/** Callback invoked when a context cleared event arrives */
export type ContextClearedCallback = (event: ContextClearedEvent) => void

export interface UseAgentStreamOptions {
  /** Called when an agent:usage_update event arrives on the stream */
  onUsageUpdate?: UsageUpdateCallback
  /** Called when an agent:context_cleared event arrives on the stream */
  onContextCleared?: ContextClearedCallback
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
  return `${wsBase}/stream/${agentId}`
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgentStream(
  agentId: string,
  options: UseAgentStreamOptions = {},
): UseAgentStreamResult {
  const [lines, setLines] = useState<LogLine[]>([])
  const [status, setStatus] = useState<StreamStatus>('connecting')

  // Store callbacks in refs so the WebSocket effect doesn't re-run when
  // callbacks change.
  const onUsageUpdateRef = useRef(options.onUsageUpdate)
  const onContextClearedRef = useRef(options.onContextCleared)
  onUsageUpdateRef.current = options.onUsageUpdate
  onContextClearedRef.current = options.onContextCleared

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

      // Try to parse as JSON event first
      let parsed: AgentEvent | null = null
      try {
        parsed = JSON.parse(rawText) as AgentEvent
      } catch {
        // Not JSON — treat as plain log output
      }

      if (parsed) {
        // Emit to the global event bus so other hooks can react
        agentEventBus.emit(parsed)

        if (parsed.type === 'agent:usage_update') {
          onUsageUpdateRef.current?.(parsed)
          return
        }

        if (parsed.type === 'agent:context_cleared') {
          onContextClearedRef.current?.(parsed)
          return
        }

        // For agent:output events, extract the line text
        if (parsed.type === 'agent:output') {
          const incoming = [makeLogLine(parsed.line)]
          setLines((prev) => capLines(prev, incoming))
          return
        }

        // Other structured events are emitted to the bus but not added
        // to the log buffer.
        return
      }

      // Plain text fallback
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
