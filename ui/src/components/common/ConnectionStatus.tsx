/**
 * ConnectionStatus — visual indicator for a WebSocket connection state.
 *
 * Renders a small coloured dot plus an optional text label.
 * Pass the `connectionState` from useWebSocket, useAllAgentsStream,
 * or useAgentStream to drive the indicator.
 *
 * States:
 *   Connected    — green dot
 *   Connecting   — yellow pulsing dot
 *   Reconnecting — yellow pulsing dot
 *   Disconnected — red dot
 */

import type { ConnectionState } from '@/services/websocket'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface ConnectionStatusProps {
  connectionState: ConnectionState
  /** Optional label override (default: state name) */
  label?: string
  /** Hide the text label — show only the dot (default: false) */
  iconOnly?: boolean
  className?: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function stateClasses(state: ConnectionState): { dot: string; text: string } {
  switch (state) {
    case 'Connected':
      return {
        dot: 'bg-green-500',
        text: 'text-green-500 dark:text-green-400',
      }
    case 'Connecting':
    case 'Reconnecting':
      return {
        dot: 'animate-pulse bg-yellow-400',
        text: 'text-yellow-500 dark:text-yellow-400',
      }
    case 'Disconnected':
      return {
        dot: 'bg-red-500',
        text: 'text-red-500 dark:text-red-400',
      }
  }
}

const STATE_LABELS: Record<ConnectionState, string> = {
  Connected: 'Connected',
  Connecting: 'Connecting',
  Reconnecting: 'Reconnecting',
  Disconnected: 'Disconnected',
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ConnectionStatus({
  connectionState,
  label,
  iconOnly = false,
  className = '',
}: ConnectionStatusProps) {
  const { dot, text } = stateClasses(connectionState)
  const displayLabel = label ?? STATE_LABELS[connectionState]

  return (
    <span
      role="status"
      aria-label={`Stream: ${displayLabel}`}
      className={`flex items-center gap-1.5 text-xs ${text} ${className}`}
    >
      <span
        aria-hidden="true"
        className={`h-2 w-2 flex-shrink-0 rounded-full ${dot}`}
      />
      {!iconOnly && <span>{displayLabel}</span>}
    </span>
  )
}

export default ConnectionStatus
