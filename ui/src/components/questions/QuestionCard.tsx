/**
 * QuestionCard — displays a single question with status and answer controls.
 *
 * Shows:
 * - Check type, asked timestamp, notification ID (linked)
 * - Status badge: Pending (yellow) / Answered (green) / Expired (gray)
 * - "Answer" button for Pending questions
 * - Submitted answer for Answered questions
 */

import { Clock, Link } from 'lucide-react'
import { StatusBadge } from '@/components/common/StatusBadge'
import type { QuestionInfo } from '@/types/ask'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface QuestionCardProps {
  question: QuestionInfo
  onAnswer: (question: QuestionInfo) => void
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatAsked(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString()
  } catch {
    return isoString
  }
}

const CHECK_TYPE_LABELS: Record<string, string> = {
  TmuxSessions: 'Tmux Sessions',
}

// ---------------------------------------------------------------------------
// QuestionCard
// ---------------------------------------------------------------------------

export function QuestionCard({ question, onAnswer }: QuestionCardProps) {
  const isPending = question.status === 'Pending'

  return (
    <div
      className={[
        'rounded-lg border bg-white dark:bg-gray-800 p-4 space-y-3',
        isPending
          ? 'border-yellow-200 dark:border-yellow-900/40'
          : 'border-gray-200 dark:border-gray-700',
      ].join(' ')}
    >
      {/* Header: check type + status */}
      <div className="flex items-start justify-between gap-2">
        <div>
          <p className="text-sm font-medium text-gray-900 dark:text-white">
            {CHECK_TYPE_LABELS[question.check_type] ?? question.check_type}
          </p>
          <p className="mt-0.5 flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
            <Clock size={11} />
            {formatAsked(question.asked_at)}
          </p>
        </div>
        <StatusBadge status={question.status} />
      </div>

      {/* Notification ID */}
      <div className="flex items-center gap-1.5">
        <Link size={11} className="text-gray-400 flex-shrink-0" />
        <span className="text-xs text-gray-400 dark:text-gray-500">Notification</span>
        <span className="font-mono text-xs text-gray-600 dark:text-gray-300 truncate">
          {question.notification_id}
        </span>
      </div>

      {/* Submitted answer (if answered) */}
      {question.answer && (
        <div className="rounded-md bg-green-50 dark:bg-green-900/10 border border-green-100 dark:border-green-900/30 px-3 py-2">
          <p className="text-xs font-medium text-green-700 dark:text-green-400">
            Answer submitted
          </p>
          <p className="mt-0.5 text-xs text-green-600 dark:text-green-300">{question.answer}</p>
        </div>
      )}

      {/* Answer button for pending questions */}
      {isPending && (
        <button
          type="button"
          onClick={() => onAnswer(question)}
          className="w-full rounded-md border border-yellow-300 dark:border-yellow-700 bg-yellow-50 dark:bg-yellow-900/20 px-3 py-1.5 text-xs font-medium text-yellow-800 dark:text-yellow-300 hover:bg-yellow-100 dark:hover:bg-yellow-900/30 transition-colors"
        >
          Answer
        </button>
      )}
    </div>
  )
}

export default QuestionCard
