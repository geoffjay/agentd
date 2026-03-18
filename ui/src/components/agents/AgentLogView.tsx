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
 * - Thinking/reasoning line display (toggleable, persisted to localStorage)
 * - Reconnection gap separator lines
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { ArrowDown, ChevronDown, ChevronRight, Eraser, Eye, EyeOff, Wifi, WifiOff, Loader2 } from 'lucide-react'
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
// localStorage key for thinking toggle preference
// ---------------------------------------------------------------------------

const THINKING_PREF_KEY = 'agentd:show-thinking'

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
// Tool use line (expandable)
// ---------------------------------------------------------------------------

interface ToolUseLineProps {
  ts: string
  toolName: string
  summary: string
  toolInput: Record<string, unknown>
}

function ToolUseLine({ ts, toolName, summary, toolInput }: ToolUseLineProps) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="my-0.5">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-start gap-2 rounded px-1 text-left hover:bg-gray-800"
      >
        <span className="flex-shrink-0 select-none text-gray-600">{ts}</span>
        <span className="flex-shrink-0 text-purple-400">
          {expanded ? (
            <ChevronDown size={12} aria-hidden="true" className="mt-0.5" />
          ) : (
            <ChevronRight size={12} aria-hidden="true" className="mt-0.5" />
          )}
        </span>
        <span className="flex items-baseline gap-1.5">
          <span className="rounded bg-purple-900/40 px-1.5 py-0.5 text-xs font-semibold text-purple-300">
            {toolName}
          </span>
          <span className="text-gray-300">{summary}</span>
        </span>
      </button>
      {expanded && (
        <div className="ml-24 mt-1 mb-2 rounded border border-gray-700 bg-gray-900 p-2 text-xs text-gray-300">
          <pre className="whitespace-pre-wrap break-all">{JSON.stringify(toolInput, null, 2)}</pre>
        </div>
      )}
    </div>
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

  // Thinking toggle — persisted to localStorage
  const [showThinking, setShowThinking] = useState<boolean>(
    () => localStorage.getItem(THINKING_PREF_KEY) === 'true',
  )

  const toggleThinking = useCallback(() => {
    setShowThinking((prev) => {
      const next = !prev
      localStorage.setItem(THINKING_PREF_KEY, String(next))
      return next
    })
  }, [])

  // Filter lines based on showThinking preference
  const visibleLines = showThinking ? lines : lines.filter((l) => !l.isThinking)

  // Auto-scroll to bottom when new lines arrive (unless scroll-locked)
  useEffect(() => {
    if (scrollLocked) return
    const el = containerRef.current
    if (el) {
      el.scrollTop = el.scrollHeight
    }
  }, [visibleLines, scrollLocked])

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
            aria-label={showThinking ? 'Hide thinking' : 'Show thinking'}
            onClick={toggleThinking}
            className="flex items-center gap-1 rounded px-2 py-0.5 text-xs text-gray-400 hover:bg-gray-700 hover:text-white"
          >
            {showThinking ? (
              <EyeOff size={12} aria-hidden="true" />
            ) : (
              <Eye size={12} aria-hidden="true" />
            )}
            {showThinking ? 'Hide thinking' : 'Show thinking'}
          </button>
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
        {visibleLines.length === 0 ? (
          <p className="text-gray-600 italic select-none">
            {status === 'connecting'
              ? 'Connecting to agent stream…'
              : status === 'disconnected'
                ? 'Stream disconnected. Reconnecting…'
                : 'Waiting for agent output…'}
          </p>
        ) : (
          visibleLines.map((line) => {
            const ts = new Date(line.timestamp).toLocaleTimeString([], {
              hour: '2-digit',
              minute: '2-digit',
              second: '2-digit',
              hour12: false,
            })

            if (line.isSeparator) {
              return (
                <div key={line.id} className="my-1 flex items-center gap-2 select-none">
                  <div className="flex-1 border-t border-dashed border-gray-700" />
                  <span className="text-xs text-gray-500 italic">{line.text}</span>
                  <div className="flex-1 border-t border-dashed border-gray-700" />
                </div>
              )
            }

            if (line.isThinking) {
              return (
                <div key={line.id} className="flex gap-2 whitespace-pre-wrap break-all italic text-blue-300/70">
                  <span className="flex-shrink-0 select-none text-gray-600">{ts}</span>
                  <span className="flex-shrink-0 select-none">💭</span>
                  <span>{stripAnsi(line.text)}</span>
                </div>
              )
            }

            if (line.toolUse) {
              return (
                <ToolUseLine
                  key={line.id}
                  ts={ts}
                  toolName={line.toolUse.tool_name}
                  summary={line.toolUse.summary}
                  toolInput={line.toolUse.tool_input}
                />
              )
            }

            return (
              <div key={line.id} className="flex gap-2 whitespace-pre-wrap break-all">
                <span className="flex-shrink-0 select-none text-gray-600">{ts}</span>
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
