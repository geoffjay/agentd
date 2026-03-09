/**
 * QuestionList — main questions page assembling all ask-service components.
 *
 * Layout:
 * - Page header with service health indicator
 * - Service connection info (ask health + notify URL)
 * - Check controls (run trigger, auto-trigger toggle)
 * - Environment status (tmux card)
 * - Questions list with status filters
 * - AnswerDialog (modal)
 */

import { useState } from 'react'
import { HelpCircle, Wifi, WifiOff, AlertTriangle, RefreshCw } from 'lucide-react'
import { useAskService } from '@/hooks/useAskService'
import { CheckControls } from '@/components/questions/CheckControls'
import { EnvironmentStatus } from '@/components/questions/EnvironmentStatus'
import { QuestionCard } from '@/components/questions/QuestionCard'
import { AnswerDialog } from '@/components/questions/AnswerDialog'
import type { QuestionInfo, QuestionStatus } from '@/types/ask'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type StatusFilter = QuestionStatus | 'All'

const STATUS_FILTERS: StatusFilter[] = ['All', 'Pending', 'Answered', 'Expired']

// ---------------------------------------------------------------------------
// QuestionList
// ---------------------------------------------------------------------------

export function QuestionList() {
  const {
    health,
    recheckHealth,
    triggering,
    lastTriggerResult,
    lastTriggerAt,
    triggerError,
    runTrigger,
    autoTrigger,
    autoTriggerInterval,
    setAutoTrigger,
    setAutoTriggerInterval,
    questions,
    answering,
    answerError,
    submitAnswer,
  } = useAskService()

  const [statusFilter, setStatusFilter] = useState<StatusFilter>('All')
  const [answerTarget, setAnswerTarget] = useState<QuestionInfo | null>(null)
  const [answerSuccess, setAnswerSuccess] = useState(false)

  const filteredQuestions =
    statusFilter === 'All'
      ? questions
      : questions.filter((q) => q.status === statusFilter)

  const pendingCount = questions.filter((q) => q.status === 'Pending').length

  const handleAnswer = (question: QuestionInfo) => {
    setAnswerTarget(question)
    setAnswerSuccess(false)
  }

  const handleSubmitAnswer = async (questionId: string, answer: string) => {
    const ok = await submitAnswer(questionId, answer)
    if (ok) {
      setAnswerSuccess(true)
      setTimeout(() => {
        setAnswerTarget(null)
        setAnswerSuccess(false)
      }, 1200)
    }
  }

  const tmux = lastTriggerResult?.results?.tmux_sessions

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div className="flex items-start justify-between gap-4 flex-wrap">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-purple-100 dark:bg-purple-900/30">
            <HelpCircle size={20} className="text-purple-600 dark:text-purple-400" />
          </div>
          <div>
            <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Questions</h1>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Pending questions waiting for your response.
            </p>
          </div>
        </div>

        {/* Service health indicator */}
        <div className="flex items-center gap-2">
          {health.checking ? (
            <div className="flex items-center gap-1.5 text-xs text-gray-400">
              <RefreshCw size={12} className="animate-spin" />
              Checking…
            </div>
          ) : health.reachable ? (
            <div className="flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400">
              <Wifi size={13} />
              Ask service · port 17001
              {health.version && (
                <span className="text-gray-400">v{health.version}</span>
              )}
            </div>
          ) : (
            <button
              type="button"
              onClick={recheckHealth}
              className="flex items-center gap-1.5 text-xs text-red-500 dark:text-red-400 hover:underline"
            >
              <WifiOff size={13} />
              Ask service unreachable — retry
            </button>
          )}
        </div>
      </div>

      {/* Notify service warning */}
      {health.reachable && !health.notifyUrl && (
        <div className="flex items-center gap-2 rounded-md border border-yellow-200 dark:border-yellow-900/40 bg-yellow-50 dark:bg-yellow-900/10 px-4 py-3 text-sm text-yellow-700 dark:text-yellow-400">
          <AlertTriangle size={14} className="flex-shrink-0" />
          <span>
            Could not determine the connected notify service URL. Answers may not be delivered.
          </span>
        </div>
      )}
      {health.reachable && health.notifyUrl && (
        <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
          <span>Connected notify service:</span>
          <code className="font-mono text-gray-600 dark:text-gray-300">{health.notifyUrl}</code>
        </div>
      )}

      {/* Main grid: controls + environment status */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <CheckControls
          triggering={triggering}
          lastTriggerResult={lastTriggerResult}
          lastTriggerAt={lastTriggerAt}
          triggerError={triggerError}
          autoTrigger={autoTrigger}
          autoTriggerInterval={autoTriggerInterval}
          onRunTrigger={runTrigger}
          onSetAutoTrigger={setAutoTrigger}
          onSetAutoTriggerInterval={setAutoTriggerInterval}
        />
        <EnvironmentStatus tmux={tmux} lastCheckedAt={lastTriggerAt} />
      </div>

      {/* Answer success toast */}
      {answerSuccess && (
        <div className="rounded-md border border-green-200 dark:border-green-900/40 bg-green-50 dark:bg-green-900/10 px-4 py-3 text-sm text-green-700 dark:text-green-400">
          Answer submitted successfully.
        </div>
      )}

      {/* Questions section */}
      <section aria-label="Questions">
        <div className="mb-3 flex items-center justify-between gap-3 flex-wrap">
          <h2 className="text-base font-semibold text-gray-900 dark:text-white">
            Questions
            {pendingCount > 0 && (
              <span className="ml-2 rounded-full bg-yellow-100 dark:bg-yellow-900/30 px-2 py-0.5 text-xs font-medium text-yellow-700 dark:text-yellow-400">
                {pendingCount} pending
              </span>
            )}
          </h2>

          {/* Status filter */}
          <div
            className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden text-xs"
            role="group"
            aria-label="Filter by status"
          >
            {STATUS_FILTERS.map((filter) => (
              <button
                key={filter}
                type="button"
                onClick={() => setStatusFilter(filter)}
                aria-pressed={statusFilter === filter}
                className={[
                  'px-3 py-1.5 transition-colors',
                  statusFilter === filter
                    ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400 font-medium'
                    : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200',
                ].join(' ')}
              >
                {filter}
              </button>
            ))}
          </div>
        </div>

        {filteredQuestions.length === 0 ? (
          <div className="rounded-lg border border-dashed border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800/50 py-12 text-center">
            <HelpCircle size={32} className="mx-auto mb-3 text-gray-300 dark:text-gray-600" />
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {statusFilter === 'All'
                ? 'No questions yet. Run checks to see if any action is needed.'
                : `No ${statusFilter.toLowerCase()} questions.`}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
            {filteredQuestions.map((question) => (
              <QuestionCard
                key={question.question_id}
                question={question}
                onAnswer={handleAnswer}
              />
            ))}
          </div>
        )}
      </section>

      {/* Answer dialog */}
      <AnswerDialog
        open={answerTarget !== null}
        question={answerTarget}
        answering={answering}
        answerError={answerError}
        onSubmit={(id, answer) => void handleSubmitAnswer(id, answer)}
        onClose={() => setAnswerTarget(null)}
      />
    </div>
  )
}

export default QuestionList
