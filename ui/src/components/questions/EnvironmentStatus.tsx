/**
 * EnvironmentStatus — displays the current tmux environment status.
 *
 * Shows:
 * - Running indicator (green/red dot)
 * - Session count and names
 * - Last checked timestamp
 * - Expandable raw JSON view
 */

import { useState } from 'react'
import { Terminal, ChevronDown, ChevronRight, Clock } from 'lucide-react'
import type { TmuxCheckResult } from '@/types/ask'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface EnvironmentStatusProps {
  tmux?: TmuxCheckResult
  lastCheckedAt?: Date
  loading?: boolean
}

// ---------------------------------------------------------------------------
// EnvironmentStatus
// ---------------------------------------------------------------------------

export function EnvironmentStatus({ tmux, lastCheckedAt, loading }: EnvironmentStatusProps) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5 space-y-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <Terminal size={15} className="text-gray-400" />
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">Environment Status</h3>
      </div>

      {loading ? (
        <div className="space-y-2">
          <div className="h-4 w-2/3 rounded bg-gray-100 dark:bg-gray-700 animate-pulse" />
          <div className="h-4 w-1/2 rounded bg-gray-100 dark:bg-gray-700 animate-pulse" />
        </div>
      ) : tmux ? (
        <div className="space-y-3">
          {/* Tmux status card */}
          <div
            className={[
              'rounded-md border p-3 space-y-2',
              tmux.running
                ? 'border-green-100 dark:border-green-900/30 bg-green-50/50 dark:bg-green-900/10'
                : 'border-red-100 dark:border-red-900/30 bg-red-50/30 dark:bg-red-900/5',
            ].join(' ')}
          >
            {/* Running indicator */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span
                  className={[
                    'h-2.5 w-2.5 rounded-full flex-shrink-0',
                    tmux.running ? 'bg-green-500' : 'bg-red-500',
                  ].join(' ')}
                  role="status"
                  aria-label={tmux.running ? 'tmux running' : 'tmux not running'}
                />
                <span className="text-sm font-medium text-gray-900 dark:text-white">
                  tmux
                </span>
              </div>
              <span
                className={[
                  'text-xs font-medium',
                  tmux.running
                    ? 'text-green-600 dark:text-green-400'
                    : 'text-red-600 dark:text-red-400',
                ].join(' ')}
              >
                {tmux.running ? 'Running' : 'Not running'}
              </span>
            </div>

            {/* Session count */}
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {tmux.session_count === 0
                ? 'No active sessions'
                : `${tmux.session_count} active session${tmux.session_count !== 1 ? 's' : ''}`}
            </p>

            {/* Session names */}
            {tmux.sessions && tmux.sessions.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {tmux.sessions.map((session) => (
                  <span
                    key={session}
                    className="rounded bg-gray-100 dark:bg-gray-700 px-1.5 py-0.5 font-mono text-xs text-gray-600 dark:text-gray-300"
                  >
                    {session}
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Last checked timestamp */}
          {lastCheckedAt && (
            <p className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
              <Clock size={11} />
              Checked at{' '}
              {lastCheckedAt.toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
              })}
            </p>
          )}

          {/* Expandable raw JSON */}
          <div>
            <button
              type="button"
              onClick={() => setExpanded((e) => !e)}
              className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
              aria-expanded={expanded}
            >
              {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              {expanded ? 'Hide' : 'Show'} raw result
            </button>
            {expanded && (
              <pre className="mt-2 overflow-auto rounded-md bg-gray-50 dark:bg-gray-900/50 border border-gray-100 dark:border-gray-700 p-2 text-xs text-gray-600 dark:text-gray-300">
                {JSON.stringify(tmux, null, 2)}
              </pre>
            )}
          </div>
        </div>
      ) : (
        <p className="text-sm text-gray-400 dark:text-gray-500">
          No check results yet. Run checks to see environment status.
        </p>
      )}
    </div>
  )
}

export default EnvironmentStatus
