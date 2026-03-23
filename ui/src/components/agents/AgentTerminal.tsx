/**
 * AgentTerminal — xterm.js web terminal for agent PTY sessions.
 *
 * Renders a full-colour terminal connected to the orchestrator's
 * /terminal/{agentId} WebSocket endpoint. Appears as the "Terminal"
 * tab alongside the existing "Logs" tab in AgentDetail.
 *
 * Props:
 *   agentId   — the agent UUID
 *   active    — true when this tab is visible; triggers lazy WS connect
 *   onViewLogs — callback to switch back to Logs tab (used in fallback state)
 *
 * States: connecting | connected | reconnecting | disconnected | unavailable
 * See: ui/design/agent-terminal-spec.md for full visual spec.
 */

import { useEffect } from 'react'
import {
  AlertTriangle,
  Keyboard,
  Loader2,
  SquareTerminal,
  Wifi,
  WifiOff,
} from 'lucide-react'
import { useAgentTerminal } from '@/hooks/useAgentTerminal'
import type { TerminalStatus } from '@/hooks/useAgentTerminal'

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function StatusBadge({ status }: { status: TerminalStatus }) {
  if (status === 'connected') {
    return (
      <span
        aria-label="Terminal connected"
        className="flex items-center gap-1 text-xs text-green-500 dark:text-green-400"
      >
        <Wifi size={12} aria-hidden="true" />
        Connected
      </span>
    )
  }
  if (status === 'connecting' || status === 'reconnecting') {
    return (
      <span
        aria-label={status === 'reconnecting' ? 'Terminal reconnecting' : 'Terminal connecting'}
        className="flex items-center gap-1 text-xs text-yellow-500 dark:text-yellow-400"
      >
        <Loader2 size={12} aria-hidden="true" className="animate-spin" />
        {status === 'reconnecting' ? 'Reconnecting…' : 'Connecting…'}
      </span>
    )
  }
  if (status === 'unavailable') {
    return (
      <span
        aria-label="PTY streaming unavailable"
        className="flex items-center gap-1 text-xs text-gray-500"
      >
        <WifiOff size={12} aria-hidden="true" />
        Unavailable
      </span>
    )
  }
  // disconnected | idle
  return (
    <span
      aria-label="Terminal disconnected"
      className="flex items-center gap-1 text-xs text-red-500 dark:text-red-400"
    >
      <WifiOff size={12} aria-hidden="true" />
      Disconnected
    </span>
  )
}

// ---------------------------------------------------------------------------
// Interactive mode toggle
// ---------------------------------------------------------------------------

interface InteractiveToggleProps {
  interactive: boolean
  onChange: (value: boolean) => void
}

function InteractiveToggle({ interactive, onChange }: InteractiveToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={interactive}
      aria-label={interactive ? 'Disable interactive mode' : 'Enable interactive mode'}
      onClick={() => onChange(!interactive)}
      className={[
        'flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors',
        interactive
          ? 'bg-primary-900/40 text-primary-400 hover:bg-primary-900/60'
          : 'text-gray-400 hover:bg-gray-800 hover:text-white',
      ].join(' ')}
    >
      <Keyboard size={11} aria-hidden="true" />
      {interactive ? 'Interactive' : 'Read-only'}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Unavailable fallback
// ---------------------------------------------------------------------------

interface TerminalUnavailableProps {
  onViewLogs: () => void
}

function TerminalUnavailable({ onViewLogs }: TerminalUnavailableProps) {
  return (
    <div
      role="status"
      aria-live="polite"
      className="flex h-full items-center justify-center bg-gray-950 p-6"
    >
      <div className="max-w-sm rounded-lg border border-gray-700 bg-gray-900 p-6 text-center">
        <SquareTerminal
          size={24}
          className="mx-auto mb-3 text-gray-500"
          aria-hidden="true"
        />
        <h3 className="mb-2 text-sm font-semibold text-gray-200">
          Terminal not available
        </h3>
        <p className="mb-4 text-sm leading-relaxed text-gray-400">
          This agent is running on the tmux or docker backend, which doesn't
          support PTY streaming. To enable the terminal view, set{' '}
          <code className="rounded bg-gray-800 px-1 py-0.5 font-mono text-xs text-gray-300">
            AGENTD_BACKEND=pty
          </code>{' '}
          when starting the agent.
        </p>
        <button
          type="button"
          onClick={onViewLogs}
          className="rounded-md px-3 py-1.5 text-xs font-medium text-gray-400 hover:bg-gray-800 hover:text-white transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-primary-500"
        >
          View Logs
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// AgentTerminal
// ---------------------------------------------------------------------------

export interface AgentTerminalProps {
  agentId: string
  /** True when this tab panel is the visible/active one */
  active: boolean
  /** Callback to switch back to the Logs tab (used in unavailable state) */
  onViewLogs: () => void
  /** Optional: notified when the terminal connection status changes */
  onStatusChange?: (status: TerminalStatus) => void
}

export function AgentTerminal({ agentId, active, onViewLogs, onStatusChange }: AgentTerminalProps) {
  const { status, containerRef, interactive, setInteractive, activate } =
    useAgentTerminal(agentId)

  // Lazy connect: activate on first render where active=true
  useEffect(() => {
    if (active) {
      activate()
    }
  }, [active, activate])

  // Notify parent of status changes (e.g. to show PTY unavailable badge)
  useEffect(() => {
    onStatusChange?.(status)
  }, [status, onStatusChange])

  return (
    <div
      aria-label="Agent terminal"
      className="flex h-full flex-col overflow-hidden rounded-lg border border-gray-700 bg-gray-950"
    >
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-gray-700 bg-gray-900 px-3 py-2">
        <StatusBadge status={status} />
        {status !== 'unavailable' && (
          <InteractiveToggle interactive={interactive} onChange={setInteractive} />
        )}
      </div>

      {/* Interactive mode warning banner */}
      {interactive && status !== 'unavailable' && (
        <div className="flex items-center gap-2 border-b border-amber-900/30 bg-amber-950/20 px-3 py-1.5 text-xs text-amber-400">
          <AlertTriangle size={12} aria-hidden="true" />
          Interactive mode — keystrokes are sent to the agent session
        </div>
      )}

      {/* Content area */}
      {status === 'unavailable' ? (
        <TerminalUnavailable onViewLogs={onViewLogs} />
      ) : (
        <div
          ref={containerRef}
          // xterm mounts its canvas here; keep visible even when tab is inactive
          // (hidden via the parent tab panel's display:none to preserve state)
          className="flex-1 overflow-hidden"
          // xterm.js handles its own focus; suppress browser outline on the container
          style={{ outline: 'none' }}
          aria-label="Terminal output"
        />
      )}
    </div>
  )
}

export default AgentTerminal
