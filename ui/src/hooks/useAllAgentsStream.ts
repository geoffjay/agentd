/**
 * useAllAgentsStream — monitors the multiplexed /stream endpoint.
 *
 * Connects to ws://<host>/stream and demultiplexes incoming JSON messages
 * by agent ID. Each message is expected to be a JSON object conforming to
 * one of the AgentEvent shapes defined in @/types/orchestrator.
 *
 * Non-JSON messages are treated as plain text output for an 'unknown' sender
 * to ensure graceful fallback.
 *
 * Returns:
 *   agentMessages   - Map of agentId → log lines (last MAX_LINES_PER_AGENT)
 *   connectionState - WebSocket connection state
 */

import { useEffect, useRef, useState } from 'react'
import { WebSocketManager } from '@/services/websocket'
import { serviceConfig } from '@/services/config'
import { agentEventBus } from '@/services/eventBus'
import type { ConnectionState } from '@/services/websocket'
import type { AgentEvent } from '@/types/orchestrator'
import type { LogLine } from './useAgentStream'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_LINES_PER_AGENT = 1_000

let msgId = 0

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function allStreamUrl(): string {
  const wsBase = serviceConfig.orchestratorServiceUrl.replace(/^http/, 'ws')
  return `${wsBase}/stream`
}

function makeLine(text: string): LogLine {
  return { id: ++msgId, text, timestamp: new Date().toISOString() }
}

function addLine(
  prev: Map<string, LogLine[]>,
  agentId: string,
  line: LogLine,
): Map<string, LogLine[]> {
  const existing = prev.get(agentId) ?? []
  const updated = [...existing, line]
  const capped =
    updated.length > MAX_LINES_PER_AGENT
      ? updated.slice(updated.length - MAX_LINES_PER_AGENT)
      : updated
  const next = new Map(prev)
  next.set(agentId, capped)
  return next
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseAllAgentsStreamResult {
  /** Map of agentId → buffered log lines from the /stream endpoint */
  agentMessages: Map<string, LogLine[]>
  connectionState: ConnectionState
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAllAgentsStream(): UseAllAgentsStreamResult {
  const [agentMessages, setAgentMessages] = useState<Map<string, LogLine[]>>(
    new Map(),
  )
  const [connectionState, setConnectionState] =
    useState<ConnectionState>('Disconnected')

  const managerRef = useRef<WebSocketManager | null>(null)

  useEffect(() => {
    const manager = new WebSocketManager(allStreamUrl())
    managerRef.current = manager

    const unsubState = manager.onStateChange(setConnectionState)

    const unsubMsg = manager.onMessage((event: MessageEvent) => {
      const raw = String(event.data)

      let parsed: AgentEvent | null = null
      try {
        parsed = JSON.parse(raw) as AgentEvent
      } catch {
        // Non-JSON fallback: treat as plain output
        setAgentMessages(prev => addLine(prev, 'unknown', makeLine(raw)))
        return
      }

      // Broadcast to all event bus subscribers
      agentEventBus.emit(parsed)

      // Maintain the per-agent log buffer for output events
      if (parsed.type === 'agent:output') {
        setAgentMessages(prev =>
          addLine(prev, parsed.agentId, makeLine(parsed.line)),
        )
      }
    })

    manager.connect()

    return () => {
      unsubState()
      unsubMsg()
      manager.disconnect()
      managerRef.current = null
    }
  }, [])

  return { agentMessages, connectionState }
}
