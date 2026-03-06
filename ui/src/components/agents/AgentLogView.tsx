/**
 * AgentLogView — terminal-style log display with WebSocket streaming.
 *
 * Features:
 * - Consumes lines from useAgentStream
 * - Auto-scroll to bottom as new lines arrive
 * - Scroll-lock: pauses auto-scroll when user scrolls up; resume button
 * - ANSI escape sequence stripping (basic colour support via CSS classes)
 * - Timestamp prefix on each line
 * - Connection status indicator
 * - Clear button
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { ArrowDown, Eraser, Wifi, WifiOff, Loader2 } from 'lucide-react'
import type { LogLine, StreamStatus } from '@/hooks/useAgentStream'

// ---------------------------------------------------------------------------
// ANSI stripping
// ---------------------------------------------------------------------------

// eslint-disable-next-line no-control-regex
const ANSI_RE = /\x1b\[[0-9;]*[A-Za-z]/g

function stripAnsi(text: string): string {
  return text.replace(ANSI_RE, '')
}

// ---------------------------------------------------------------------------
// Status indicator
// ---------------------------------------------------------------------------

function StreamStatusBadge({ status }: { status: StreamStatus }) {
  if (status === 'connected') {
    return (
      <span
        aria-label="Stream connected"
        className="flex items-center gap-1 text-xs text-green-500 dark:text-green-400"
      >
        <Wifi size={12} aria-hidden="true" />
        Connected
      </span>
    )
  }
  if (status === 'connecting') {
    return (
      <span
        aria-label="Stream connecting"
        className="flex items-center gap-1 text-xs text-yellow-500 dark:text-yellow-400"
      >
        <Loader2 size={12} aria-hidden="true" className="animate-spin" />
        Connecting…
      </span>
    )
  }
  return (
    <span
      aria-label="Stream disconnected"
      className="flex items-center gap-1 text-xs text-red-500 dark:text-red-400"
    >
      <WifiOff size={12} aria-hidden="true" />
      Disconnected
    </span>
  )
}

// ---------------------------------------------------------------------------
// AgentLogView
// ---------------------------------------------------------------------------

export interface AgentLogViewProps {
  lines: LogLine[]
  status: StreamStatus
  onClear: () => void
}

export function AgentLogView({ lines, status, onClear }: AgentLogViewProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [scrollLocked, setScrollLocked] = useState(false)

  // Auto-scroll to bottom when new lines arrive (unless scroll-locked)
  useEffect(() => {
    if (scrollLocked) return
    const el = containerRef.current
    if (el) {
      el.scrollTop = el.scrollHeight
    }
  }, [lines, scrollLocked])

  // Detect when user scrolls up — engage scroll lock
  const handleScroll = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40
    setScrollLocked(!atBottom)
  }, [])

  // Resume auto-scroll: jump to bottom and clear lock
  const resumeScroll = useCallback(() => {
    setScrollLocked(false)
    const el = containerRef.current
    if (el) {
      el.scrollTop = el.scrollHeight
    }
  }, [])

  return (
    <div
      aria-label="Agent log output"
      className="flex h-full flex-col overflow-hidden rounded-lg border border-gray-700 bg-gray-950"
    >
      {/* Toolbar */}
      <div className="flex items-center justify-between border-b border-gray-700 bg-gray-900 px-3 py-2">
        <StreamStatusBadge status={status} />
        <div className="flex items-center gap-2">
          {scrollLocked && (
            <button
              type="button"
              aria-label="Resume auto-scroll"
              onClick={resumeScroll}
              className="flex items-center gap-1 rounded px-2 py-0.5 text-xs text-yellow-400 hover:bg-gray-700"
            >
              <ArrowDown size={12} aria-hidden="true" />
              Resume scroll
            </button>
          )}
          <button
            type="button"
            aria-label="Clear log"
            onClick={onClear}
            className="flex items-center gap-1 rounded px-2 py-0.5 text-xs text-gray-400 hover:bg-gray-700 hover:text-white"
          >
            <Eraser size={12} aria-hidden="true" />
            Clear
          </button>
        </div>
      </div>

      {/* Log lines */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto px-3 py-2 font-mono text-xs leading-5 text-gray-200"
        aria-live="polite"
        aria-atomic="false"
        aria-relevant="additions"
      >
        {lines.length === 0 ? (
          <p className="text-gray-600 italic select-none">
            {status === 'connecting'
              ? 'Connecting to agent stream…'
              : status === 'disconnected'
                ? 'Stream disconnected. Reconnecting…'
                : 'Waiting for agent output…'}
          </p>
        ) : (
          lines.map(line => {
            const ts = new Date(line.timestamp).toLocaleTimeString([], {
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
              hour12: false,
            })
            return (
              <div key={line.id} className="flex gap-2 whitespace-pre-wrap break-all">
                <span className="flex-shrink-0 select-none text-gray-600">
                  {ts}
                </span>
                <span>{stripAnsi(line.text)}</span>
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}

export default AgentLogView
