/**
 * CheckControls — run environment checks manually or on a schedule.
 *
 * Shows:
 * - "Run Checks" button with loading state
 * - Last trigger timestamp
 * - Results after a successful trigger (checks run, notifications, tmux details)
 * - Auto-trigger toggle with interval picker
 */

import { Play, RefreshCw, Clock, Bell, ToggleLeft, ToggleRight } from 'lucide-react'
import type { TriggerResponse } from '@/types/ask'
import type { AutoTriggerInterval } from '@/hooks/useAskService'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface CheckControlsProps {
  triggering: boolean
  lastTriggerResult?: TriggerResponse
  lastTriggerAt?: Date
  triggerError?: string
  autoTrigger: boolean
  autoTriggerInterval: AutoTriggerInterval
  onRunTrigger: () => void
  onSetAutoTrigger: (enabled: boolean) => void
  onSetAutoTriggerInterval: (ms: AutoTriggerInterval) => void
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const INTERVAL_LABELS: Record<number, string> = {
  30_000: '30s',
  60_000: '1m',
  300_000: '5m',
  600_000: '10m',
}

function formatTimestamp(date: Date): string {
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

// ---------------------------------------------------------------------------
// CheckControls
// ---------------------------------------------------------------------------

export function CheckControls({
  triggering,
  lastTriggerResult,
  lastTriggerAt,
  triggerError,
  autoTrigger,
  autoTriggerInterval,
  onRunTrigger,
  onSetAutoTrigger,
  onSetAutoTriggerInterval,
}: CheckControlsProps) {
  const tmux = lastTriggerResult?.results?.tmux_sessions

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div>
          <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
            Environment Checks
          </h3>
          {lastTriggerAt && (
            <p className="mt-0.5 flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
              <Clock size={11} />
              Last run at {formatTimestamp(lastTriggerAt)}
            </p>
          )}
        </div>

        {/* Run button */}
        <button
          type="button"
          onClick={onRunTrigger}
          disabled={triggering}
          aria-label="Run environment checks"
          className="flex items-center gap-2 rounded-md bg-primary-600 hover:bg-primary-700 disabled:opacity-60 px-4 py-2 text-sm font-medium text-white transition-colors"
        >
          {triggering ? (
            <RefreshCw size={14} className="animate-spin" />
          ) : (
            <Play size={14} />
          )}
          {triggering ? 'Running…' : 'Run Checks'}
        </button>
      </div>

      {/* Error */}
      {triggerError && (
        <div className="rounded-md border border-red-200 dark:border-red-900/40 bg-red-50 dark:bg-red-900/10 px-3 py-2 text-sm text-red-600 dark:text-red-400">
          {triggerError}
        </div>
      )}

      {/* Last trigger results */}
      {lastTriggerResult && (
        <div className="space-y-3">
          {/* Checks performed */}
          <div className="flex items-start gap-2">
            <RefreshCw size={13} className="mt-0.5 text-gray-400 flex-shrink-0" />
            <div>
              <p className="text-xs font-medium text-gray-600 dark:text-gray-400">Checks run</p>
              <div className="mt-1 flex flex-wrap gap-1">
                {lastTriggerResult.checks_run.map((check) => (
                  <span
                    key={check}
                    className="rounded-full bg-blue-100 dark:bg-blue-900/30 px-2 py-0.5 text-xs text-blue-700 dark:text-blue-300"
                  >
                    {check}
                  </span>
                ))}
              </div>
            </div>
          </div>

          {/* Notifications sent */}
          <div className="flex items-start gap-2">
            <Bell size={13} className="mt-0.5 text-gray-400 flex-shrink-0" />
            <div>
              <p className="text-xs font-medium text-gray-600 dark:text-gray-400">
                Notifications sent
              </p>
              {lastTriggerResult.notifications_sent.length === 0 ? (
                <p className="mt-0.5 text-xs text-gray-400 dark:text-gray-500">None</p>
              ) : (
                <ul className="mt-1 space-y-0.5">
                  {lastTriggerResult.notifications_sent.map((id) => (
                    <li key={id} className="font-mono text-xs text-gray-500 dark:text-gray-400">
                      {id}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>

          {/* Tmux result detail */}
          {tmux && (
            <div className="rounded-md bg-gray-50 dark:bg-gray-900/50 border border-gray-100 dark:border-gray-700 px-3 py-2 space-y-1">
              <p className="text-xs font-medium text-gray-600 dark:text-gray-400">
                tmux_sessions result
              </p>
              <div className="flex items-center gap-2">
                <span
                  className={[
                    'h-2 w-2 rounded-full flex-shrink-0',
                    tmux.running ? 'bg-green-500' : 'bg-red-500',
                  ].join(' ')}
                />
                <span className="text-xs text-gray-600 dark:text-gray-300">
                  {tmux.running
                    ? `Running — ${tmux.session_count} session${tmux.session_count !== 1 ? 's' : ''}`
                    : 'Not running'}
                </span>
              </div>
              {tmux.sessions && tmux.sessions.length > 0 && (
                <p className="text-xs text-gray-400 dark:text-gray-500 pl-4">
                  {tmux.sessions.join(', ')}
                </p>
              )}
            </div>
          )}
        </div>
      )}

      {/* Auto-trigger controls */}
      <div className="border-t border-gray-100 dark:border-gray-700 pt-4 flex items-center justify-between gap-3 flex-wrap">
        <div className="flex items-center gap-2">
          <button
            type="button"
            role="switch"
            aria-checked={autoTrigger}
            onClick={() => onSetAutoTrigger(!autoTrigger)}
            className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors"
          >
            {autoTrigger ? (
              <ToggleRight size={20} className="text-primary-600 dark:text-primary-400" />
            ) : (
              <ToggleLeft size={20} className="text-gray-400" />
            )}
            Auto-trigger
          </button>
        </div>

        {autoTrigger && (
          <div
            className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden text-xs"
            role="group"
            aria-label="Auto-trigger interval"
          >
            {Object.entries(INTERVAL_LABELS).map(([ms, label]) => (
              <button
                key={ms}
                type="button"
                onClick={() => onSetAutoTriggerInterval(Number(ms) as AutoTriggerInterval)}
                aria-pressed={autoTriggerInterval === Number(ms)}
                className={[
                  'px-2.5 py-1 transition-colors',
                  autoTriggerInterval === Number(ms)
                    ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400 font-medium'
                    : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200',
                ].join(' ')}
              >
                {label}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

export default CheckControls
