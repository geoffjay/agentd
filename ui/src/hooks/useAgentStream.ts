/**
 * useAgentStream — WebSocket hook for real-time agent log streaming.
 *
 * Connects to ws://<host>/stream/<agentId> via WebSocketManager, which
 * handles auto-reconnect (exponential backoff), heartbeat detection,
 * and message buffering.
 *
 * Returns a capped circular buffer of up to MAX_LINES log lines plus
 * a connection status indicator. Log history is persisted to sessionStorage
 * so it survives component remounts within the same browser tab. A separator
 * line is injected when history is rehydrated to mark any gap in output.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import { serviceConfig } from '@/services/config'
import { agentEventBus } from '@/services/eventBus'
import type { AgentEvent, UsageUpdateEvent, ContextClearedEvent, AgentToolUseEvent, AgentThinkingEvent } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type StreamStatus = 'connecting' | 'connected' | 'disconnected'

export interface LogLine {
  id: number
  /** Raw text (may contain ANSI escape sequences) */
  text: string
  timestamp: string
  /** When set, this line represents a tool call rather than plain output */
  toolUse?: {
    tool_name: string
    tool_id: string
    tool_input: Record<string, unknown>
    summary: string
  }
  /** When true, this line is a thinking/reasoning block */
  isThinking?: boolean
  /** When true, this line is a reconnection gap separator */
  isSeparator?: boolean
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
const MAX_STORED_LINES = 1_000

// ---------------------------------------------------------------------------
// sessionStorage helpers
// ---------------------------------------------------------------------------

const LOG_STORAGE_KEY = (agentId: string) => `agentd:log-history:${agentId}`

interface StoredLogHistory {
  lines: LogLine[]
  lastTimestamp: string
}

/** Deduplicate lines by id, keeping the first occurrence. */
function deduplicateLines(lines: LogLine[]): LogLine[] {
  const seen = new Set<number>()
  return lines.filter((l) => {
    if (seen.has(l.id)) return false
    seen.add(l.id)
    return true
  })
}

function loadLogHistory(agentId: string): StoredLogHistory | null {
  try {
    const raw = sessionStorage.getItem(LOG_STORAGE_KEY(agentId))
    if (!raw) return null
    const history = JSON.parse(raw) as StoredLogHistory
    // Guard against corrupt storage with duplicate IDs
    history.lines = deduplicateLines(history.lines)
    return history
  } catch {
    return null
  }
}

function saveLogHistory(agentId: string, lines: LogLine[], lastTimestamp: string): void {
  try {
    const stored: StoredLogHistory = { lines: deduplicateLines(lines), lastTimestamp }
    sessionStorage.setItem(LOG_STORAGE_KEY(agentId), JSON.stringify(stored))
  } catch {
    // sessionStorage may be full or unavailable — silently ignore
  }
}

function clearLogHistory(agentId: string): void {
  try {
    sessionStorage.removeItem(LOG_STORAGE_KEY(agentId))
  } catch {
    // ignore
  }
}

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

function makeToolUseLine(event: AgentToolUseEvent): LogLine {
  return {
    id: ++globalLineId,
    text: `[${event.tool_name}] ${event.summary}`,
    timestamp: event.timestamp,
    toolUse: {
      tool_name: event.tool_name,
      tool_id: event.tool_id,
      tool_input: event.tool_input,
      summary: event.summary,
    },
  }
}

function makeThinkingLine(text: string, timestamp: string): LogLine {
  return {
    id: ++globalLineId,
    text,
    timestamp,
    isThinking: true,
  }
}

function makeSeparatorLine(fromTs: string, toTs: string): LogLine {
  const fmt = (ts: string) =>
    new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })
  return {
    id: ++globalLineId,
    text: `─── Reconnected · missed output from ${fmt(fromTs)} to ${fmt(toTs)} ───`,
    timestamp: toTs,
    isSeparator: true,
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

  // linesRef stays in sync with lines state for use in cleanup
  const linesRef = useRef<LogLine[]>([])

  // Stores the last stored timestamp when history is rehydrated; cleared
  // after the first new message arrives (so we can insert a separator)
  const pendingSeparatorRef = useRef<string | null>(null)

  // Store callbacks in refs so the WebSocket effect doesn't re-run when
  // callbacks change.
  const onUsageUpdateRef = useRef(options.onUsageUpdate)
  const onContextClearedRef = useRef(options.onContextCleared)
  onUsageUpdateRef.current = options.onUsageUpdate
  onContextClearedRef.current = options.onContextCleared

  const managerRef = useRef<WebSocketManager | null>(null)

  useEffect(() => {
    // Rehydrate persisted log history on mount
    const stored = loadLogHistory(agentId)
    if (stored && stored.lines.length > 0) {
      // Advance globalLineId past any rehydrated IDs so new lines never
      // collide with stored ones (globalLineId resets to 0 on page reload).
      const maxStoredId = stored.lines.reduce((max, l) => Math.max(max, l.id), 0)
      if (maxStoredId >= globalLineId) {
        globalLineId = maxStoredId
      }
      setLines((prev) => {
        const next = capLines(prev, stored.lines)
        linesRef.current = next
        return next
      })
      pendingSeparatorRef.current = stored.lastTimestamp
    }

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
          const newLine = makeLogLine(parsed.line)
          const separator = pendingSeparatorRef.current
          pendingSeparatorRef.current = null
          setLines((prev) => {
            const incoming = separator
              ? [makeSeparatorLine(separator, newLine.timestamp), newLine]
              : [newLine]
            const next = capLines(prev, incoming)
            linesRef.current = next
            return next
          })
          return
        }

        // For agent:tool_use events, add a structured tool call line
        if (parsed.type === 'agent:tool_use') {
          const newLine = makeToolUseLine(parsed)
          const separator = pendingSeparatorRef.current
          pendingSeparatorRef.current = null
          setLines((prev) => {
            const incoming = separator
              ? [makeSeparatorLine(separator, newLine.timestamp), newLine]
              : [newLine]
            const next = capLines(prev, incoming)
            linesRef.current = next
            return next
          })
          return
        }

        // For agent:thinking events, add a thinking log line
        if (parsed.type === 'agent:thinking') {
          const thinkingEvent = parsed as AgentThinkingEvent
          const newLine = makeThinkingLine(thinkingEvent.text, thinkingEvent.timestamp)
          const separator = pendingSeparatorRef.current
          pendingSeparatorRef.current = null
          setLines((prev) => {
            const incoming = separator
              ? [makeSeparatorLine(separator, newLine.timestamp), newLine]
              : [newLine]
            const next = capLines(prev, incoming)
            linesRef.current = next
            return next
          })
          return
        }

        // Other structured events are emitted to the bus but not added
        // to the log buffer.
        return
      }

      // Plain text fallback
      const separator = pendingSeparatorRef.current
      pendingSeparatorRef.current = null
      setLines((prev) => {
        const newLines = rawText.split('\n').filter(Boolean).map(makeLogLine)
        const incoming = separator && newLines.length > 0
          ? [makeSeparatorLine(separator, newLines[0].timestamp), ...newLines]
          : newLines
        const next = capLines(prev, incoming)
        linesRef.current = next
        return next
      })
    })

    manager.connect()

    return () => {
      unsubState()
      unsubMsg()
      manager.disconnect()
      managerRef.current = null

      // Persist log history (excluding separator lines) on unmount
      const currentLines = linesRef.current.filter((l) => !l.isSeparator)
      const trimmed = currentLines.slice(-MAX_STORED_LINES)
      if (trimmed.length > 0) {
        const lastTimestamp = trimmed[trimmed.length - 1].timestamp
        saveLogHistory(agentId, trimmed, lastTimestamp)
      }
    }
  }, [agentId])

  const clear = useCallback(() => {
    linesRef.current = []
    pendingSeparatorRef.current = null
    setLines([])
    clearLogHistory(agentId)
  }, [agentId])

  return { lines, status, clear }
}
