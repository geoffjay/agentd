/**
 * AnswerDialog — modal for submitting an answer to a pending question.
 *
 * Shows:
 * - Question context (check type, what triggered it)
 * - Text input for free-form answer
 * - Quick-answer buttons for common responses
 * - Submit via POST /answer
 * - Success/error feedback
 */

import { useEffect, useRef, useState } from 'react'
import { X, MessageSquare, Zap } from 'lucide-react'
import type { QuestionInfo } from '@/types/ask'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AnswerDialogProps {
  open: boolean
  question: QuestionInfo | null
  answering: boolean
  answerError?: string
  onSubmit: (questionId: string, answer: string) => void
  onClose: () => void
}

// ---------------------------------------------------------------------------
// Quick answers
// ---------------------------------------------------------------------------

const QUICK_ANSWERS = [
  { label: 'Yes, start sessions', value: 'yes' },
  { label: 'No, ignore for now', value: 'no' },
  { label: 'Acknowledged', value: 'acknowledged' },
]

// ---------------------------------------------------------------------------
// AnswerDialog
// ---------------------------------------------------------------------------

export function AnswerDialog({
  open,
  question,
  answering,
  answerError,
  onSubmit,
  onClose,
}: AnswerDialogProps) {
  const [answer, setAnswer] = useState('')
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)

  // Reset answer text when dialog opens for a new question
  useEffect(() => {
    if (open) {
      setAnswer('')
      // Focus the textarea after the dialog opens
      setTimeout(() => inputRef.current?.focus(), 50)
    }
  }, [open, question?.question_id])

  // Close on Escape
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' && open) onClose()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [open, onClose])

  if (!open || !question) return null

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!answer.trim()) return
    onSubmit(question.question_id, answer.trim())
  }

  const handleQuickAnswer = (value: string) => {
    onSubmit(question.question_id, value)
  }

  const checkLabel =
    question.check_type === 'TmuxSessions' ? 'tmux session check' : question.check_type

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-40 bg-black/40 backdrop-blur-sm"
        aria-hidden="true"
        onClick={onClose}
      />

      {/* Dialog */}
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="answer-dialog-title"
        className="fixed inset-0 z-50 flex items-center justify-center p-4"
      >
        <div className="relative w-full max-w-lg rounded-xl bg-white dark:bg-gray-800 shadow-xl border border-gray-200 dark:border-gray-700 p-6 space-y-4">
          {/* Close button */}
          <button
            type="button"
            onClick={onClose}
            className="absolute right-4 top-4 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors"
            aria-label="Close answer dialog"
          >
            <X size={18} />
          </button>

          {/* Title */}
          <div className="flex items-center gap-2.5">
            <div className="flex h-9 w-9 items-center justify-center rounded-full bg-yellow-100 dark:bg-yellow-900/30">
              <MessageSquare size={17} className="text-yellow-600 dark:text-yellow-400" />
            </div>
            <div>
              <h2
                id="answer-dialog-title"
                className="text-base font-semibold text-gray-900 dark:text-white"
              >
                Answer Question
              </h2>
              <p className="text-xs text-gray-400 dark:text-gray-500">
                From {checkLabel}
              </p>
            </div>
          </div>

          {/* Context */}
          <div className="rounded-md bg-gray-50 dark:bg-gray-900/50 border border-gray-100 dark:border-gray-700 p-3 space-y-1.5 text-xs">
            <div className="flex justify-between">
              <span className="text-gray-400 dark:text-gray-500">Check type</span>
              <span className="font-medium text-gray-700 dark:text-gray-300">
                {question.check_type}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-400 dark:text-gray-500">Asked at</span>
              <span className="font-medium text-gray-700 dark:text-gray-300">
                {new Date(question.asked_at).toLocaleString()}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-400 dark:text-gray-500">Notification</span>
              <span className="font-mono font-medium text-gray-700 dark:text-gray-300 truncate max-w-[180px]">
                {question.notification_id}
              </span>
            </div>
          </div>

          {/* Quick answers */}
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-gray-500 dark:text-gray-400 flex items-center gap-1">
              <Zap size={11} /> Quick answers
            </p>
            <div className="flex flex-wrap gap-2">
              {QUICK_ANSWERS.map((qa) => (
                <button
                  key={qa.value}
                  type="button"
                  disabled={answering}
                  onClick={() => handleQuickAnswer(qa.value)}
                  className="rounded-full border border-gray-200 dark:border-gray-600 px-3 py-1 text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
                >
                  {qa.label}
                </button>
              ))}
            </div>
          </div>

          {/* Free-form answer */}
          <form onSubmit={handleSubmit} className="space-y-3">
            <div>
              <label
                htmlFor="answer-input"
                className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                Custom answer
              </label>
              <textarea
                id="answer-input"
                ref={inputRef}
                rows={3}
                value={answer}
                onChange={(e) => setAnswer(e.target.value)}
                placeholder="Type your answer…"
                disabled={answering}
                className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-3 py-2 text-sm text-gray-900 dark:text-white placeholder:text-gray-400 focus:outline-none focus:ring-2 focus:ring-primary-500 disabled:opacity-50 resize-none"
              />
            </div>

            {/* Error */}
            {answerError && (
              <p className="text-xs text-red-600 dark:text-red-400">{answerError}</p>
            )}

            {/* Actions */}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={onClose}
                disabled={answering}
                className="rounded-md border border-gray-200 dark:border-gray-700 px-4 py-2 text-sm text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!answer.trim() || answering}
                className="rounded-md bg-primary-600 hover:bg-primary-700 disabled:opacity-50 px-4 py-2 text-sm font-medium text-white transition-colors"
              >
                {answering ? 'Submitting…' : 'Submit Answer'}
              </button>
            </div>
          </form>
        </div>
      </div>
    </>
  )
}

export default AnswerDialog
