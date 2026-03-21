/**
 * PipelineStatusCard — dashboard widget for the autonomous pipeline (v0.10.0).
 *
 * Shows:
 *   • Merge queue — PRs labeled merge-queue, ordered bottom-of-stack first
 *   • Stack stats — active stack count, stale PR count
 *   • Last sync — timestamp of the conductor's most recent git-spice repo sync
 *
 * States:
 *   loading   — skeleton placeholder while fetching
 *   empty     — conductor not yet active; shown when status is null
 *   populated — live merge queue and stats
 *
 * The card is intentionally narrow in scope: it surfaces actionable pipeline
 * state at a glance. Deep stack inspection (dependency graph, per-PR diffs)
 * belongs on a future /pipeline detail page.
 *
 * Accessibility: section landmark with labelled heading; all status
 * indicators carry aria-labels; reduced-motion respected via CSS.
 */

import { CheckCircle2, Circle, GitBranch, Loader2, RefreshCw, Workflow, XCircle } from 'lucide-react'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import type { PipelineQueueItem, PipelineStatus } from '@/types/pipeline'

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/** Indented stack-depth prefix for queue items */
function StackPrefix({ depth }: { depth: number }) {
  if (depth === 0) return null
  return (
    <span aria-hidden="true" className="select-none text-gray-300 dark:text-gray-600">
      {'  '.repeat(depth - 1)}└{' '}
    </span>
  )
}

/** CI status indicator */
function CiIndicator({ passing }: { passing: boolean | null }) {
  if (passing === null) {
    return (
      <Loader2
        size={13}
        aria-label="CI running"
        className="animate-spin text-gray-400 motion-reduce:animate-none"
      />
    )
  }
  if (passing) {
    return <CheckCircle2 size={13} aria-label="CI passing" className="text-green-500" />
  }
  return <XCircle size={13} aria-label="CI failing" className="text-red-500" />
}

/** Single row in the merge queue */
function QueueRow({ item }: { item: PipelineQueueItem }) {
  return (
    <li className="flex items-center gap-2 py-1 text-sm">
      {/* Stack depth prefix */}
      <span className="font-mono text-xs text-gray-400 dark:text-gray-500">
        <StackPrefix depth={item.stackDepth} />
        <span className="text-gray-500 dark:text-gray-400">#{item.prNumber}</span>
      </span>

      {/* Title */}
      <span
        className="min-w-0 flex-1 truncate text-gray-800 dark:text-gray-200"
        title={item.title}
      >
        {item.title}
      </span>

      {/* Approval dot */}
      <span
        role="img"
        aria-label={item.approved ? 'Approved' : 'Awaiting approval'}
        title={item.approved ? 'Approved' : 'Awaiting approval'}
        className={[
          'h-2 w-2 shrink-0 rounded-full',
          item.approved ? 'bg-green-500' : 'bg-gray-300 dark:bg-gray-600',
        ].join(' ')}
      />

      {/* CI status */}
      <span className="shrink-0">
        <CiIndicator passing={item.ciPassing} />
      </span>
    </li>
  )
}

/** Empty state shown when no conductor has run yet */
function EmptyState() {
  return (
    <div className="flex flex-col items-center gap-3 py-8 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-full bg-purple-50 dark:bg-purple-900/20">
        <Workflow size={22} className="text-purple-400 dark:text-purple-500" aria-hidden="true" />
      </div>
      <div>
        <p className="text-sm font-medium text-gray-700 dark:text-gray-300">
          Conductor not yet active
        </p>
        <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
          Deploy{' '}
          <code className="rounded bg-gray-100 px-1 py-0.5 font-mono dark:bg-gray-700">
            conductor.yml
          </code>{' '}
          to activate the autonomous pipeline.
        </p>
      </div>
    </div>
  )
}

/** Populated state with merge queue and stats */
function PipelineContent({ status }: { status: PipelineStatus }) {
  const { mergeQueue, activeStackCount, staleCount } = status

  return (
    <>
      {/* Merge queue */}
      <div className="mt-4">
        <div className="mb-2 flex items-center gap-2">
          <GitBranch size={13} aria-hidden="true" className="text-gray-400" />
          <span className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
            Merge Queue
          </span>
          {mergeQueue.length > 0 && (
            <span className="ml-auto rounded-full bg-purple-100 px-2 py-0.5 text-xs font-medium text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
              {mergeQueue.length}
            </span>
          )}
        </div>

        {mergeQueue.length === 0 ? (
          <p className="py-3 text-center text-sm text-gray-400 dark:text-gray-500">
            Queue is empty
          </p>
        ) : (
          <ul
            role="list"
            aria-label="Merge queue"
            className="divide-y divide-gray-50 dark:divide-gray-700/50"
          >
            {mergeQueue.slice(0, 6).map((item) => (
              <QueueRow key={item.prNumber} item={item} />
            ))}
            {mergeQueue.length > 6 && (
              <li className="py-1 text-center text-xs text-gray-400 dark:text-gray-500">
                +{mergeQueue.length - 6} more
              </li>
            )}
          </ul>
        )}
      </div>

      {/* Stats row */}
      <div className="mt-4 flex items-center gap-4 border-t border-gray-100 pt-3 text-xs text-gray-500 dark:border-gray-700 dark:text-gray-400">
        <span className="flex items-center gap-1">
          <Circle size={8} className="fill-purple-400 text-purple-400" aria-hidden="true" />
          {activeStackCount} active {activeStackCount === 1 ? 'stack' : 'stacks'}
        </span>
        {staleCount > 0 && (
          <span className="flex items-center gap-1 text-amber-600 dark:text-amber-400">
            <Circle size={8} className="fill-amber-400 text-amber-400" aria-hidden="true" />
            {staleCount} stale
          </span>
        )}
      </div>
    </>
  )
}

// ---------------------------------------------------------------------------
// PipelineStatusCard
// ---------------------------------------------------------------------------

export interface PipelineStatusCardProps {
  status: PipelineStatus | null
  loading: boolean
  error?: string
  onRefetch?: () => void
}

export function PipelineStatusCard({
  status,
  loading,
  error,
  onRefetch,
}: PipelineStatusCardProps) {
  const lastSync = status?.lastSyncAt ? formatRelativeTime(new Date(status.lastSyncAt)) : null

  return (
    <section
      aria-labelledby="pipeline-status-heading"
      className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2
          id="pipeline-status-heading"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          Pipeline
        </h2>

        <div className="flex items-center gap-3">
          {lastSync && (
            <span className="text-xs text-gray-400 dark:text-gray-500">
              Synced {lastSync}
            </span>
          )}
          {onRefetch && (
            <button
              type="button"
              onClick={onRefetch}
              aria-label="Refresh pipeline status"
              className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
            >
              <RefreshCw size={13} />
            </button>
          )}
        </div>
      </div>

      {/* Error */}
      {error && (
        <p role="alert" className="mt-3 text-sm text-red-500 dark:text-red-400">
          {error}
        </p>
      )}

      {/* Loading */}
      {loading && !error && (
        <div className="mt-4">
          <CardSkeleton />
        </div>
      )}

      {/* Content */}
      {!loading && !error && (
        status ? <PipelineContent status={status} /> : <EmptyState />
      )}
    </section>
  )
}

export default PipelineStatusCard

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin} min ago`
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return `${diffHr}h ago`
  return `${Math.floor(diffHr / 24)}d ago`
}
