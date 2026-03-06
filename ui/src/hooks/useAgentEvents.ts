/**
 * useAgentEvents — subscribes to typed events emitted by the central
 * agentEventBus.
 *
 * Events are produced by useAllAgentsStream as it parses WebSocket messages
 * from the /stream endpoint.
 *
 * Usage:
 *   const { subscribe } = useAgentEvents()
 *
 *   useEffect(() => {
 *     return subscribe('agent:status_change', event => {
 *       console.log(event.agentId, event.status)
 *     })
 *   }, [subscribe])
 */

import { useCallback, useEffect, useRef } from 'react'
import { agentEventBus } from '@/services/eventBus'
import type { AgentEvent } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseAgentEventsResult {
  /**
   * Subscribe to a specific event type.
   * @returns Cleanup function — store and call in useEffect cleanup.
   */
  subscribe: <T extends AgentEvent>(
    type: T['type'],
    handler: (event: T) => void,
  ) => () => void
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAgentEvents(): UseAgentEventsResult {
  // Track active subscriptions for cleanup on unmount
  const cleanupFns = useRef<Array<() => void>>([])

  useEffect(() => {
    return () => {
      for (const fn of cleanupFns.current) {
        fn()
      }
      cleanupFns.current = []
    }
  }, [])

  const subscribe = useCallback(
    <T extends AgentEvent>(type: T['type'], handler: (event: T) => void) => {
      const cleanup = agentEventBus.on(type, handler)
      cleanupFns.current.push(cleanup)
      return cleanup
    },
    [],
  )

  return { subscribe }
}
