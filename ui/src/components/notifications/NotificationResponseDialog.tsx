/**
 * NotificationResponseDialog — modal for responding to actionable notifications.
 *
 * Shows full notification title, message, source details and a text area
 * for the user's response. On submit calls PUT /notifications/{id} with
 * status "Responded" and the response text.
 */

import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import type { Notification } from '@/types/notify'

const SOURCE_LABELS: Record<string, string> = {
  System: 'System',
  AskService: 'Ask Service',
  AgentHook: 'Agent Hook',
  MonitorService: 'Monitor Service',
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface NotificationResponseDialogProps {
  notification: Notification | null
  /** Whether the submit action is in progress */
  busy?: boolean
  onSubmit: (id: string, response: string) => Promise<void>
  onClose: () => void
}

export function NotificationResponseDialog({
  notification,
  busy = false,
  onSubmit,
  onClose,
}: NotificationResponseDialogProps) {
  const [responseText, setResponseText] = useState('')
  const [error, setError] = useState<string | undefined>()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)

  // Reset when notification changes
  useEffect(() => {
    setResponseText('')
    setError(undefined)
  }, [notification?.id])

  // Focus textarea when dialog opens
  useEffect(() => {
    if (notification) {
      setTimeout(() => textareaRef.current?.focus(), 50)
    }
  }, [notification])

  // Close on Escape
  useEffect(() => {
    if (!notification) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [notification, onClose])

  if (!notification) return null

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    const trimmed = responseText.trim()
    if (!trimmed) {
      setError('Response cannot be empty.')
      return
    }
    setError(undefined)
    try {
      await onSubmit(notification.id, trimmed)
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to submit response.')
    }
  }

  return (
    /* Backdrop */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      aria-modal="true"
      role="dialog"
      aria-labelledby="response-dialog-title"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div
        ref={dialogRef}
        className="w-full max-w-lg rounded-xl border border-gray-700 bg-gray-900 shadow-2xl"
      >
        {/* Header */}
        <div className="flex items-start justify-between gap-4 border-b border-gray-700 px-6 py-4">
          <div className="min-w-0">
            <h2
              id="response-dialog-title"
              className="text-base font-semibold text-white truncate"
            >
              {notification.title}
            </h2>
            <p className="mt-0.5 text-xs text-gray-400">
              {SOURCE_LABELS[notification.source] ?? notification.source}
              {' · '}
              {notification.priority} priority
            </p>
          </div>
          <button
            type="button"
            aria-label="Close dialog"
            onClick={onClose}
            className="shrink-0 rounded-md p-1 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="px-6 py-4 space-y-4">
          {/* Full message */}
          <div className="rounded-md bg-gray-800 p-4 text-sm text-gray-300 whitespace-pre-wrap max-h-40 overflow-y-auto">
            {notification.message}
          </div>

          {/* Response form */}
          <form onSubmit={handleSubmit} className="space-y-3">
            <label htmlFor="response-input" className="block text-sm font-medium text-gray-300">
              Your response
            </label>
            <textarea
              id="response-input"
              ref={textareaRef}
              value={responseText}
              onChange={(e) => setResponseText(e.target.value)}
              rows={4}
              placeholder="Type your response here…"
              disabled={busy}
              className="w-full rounded-md border border-gray-600 bg-gray-800 px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 disabled:opacity-50 resize-none"
            />

            {error && (
              <p role="alert" className="text-xs text-red-400">
                {error}
              </p>
            )}

            {/* Actions */}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={onClose}
                disabled={busy}
                className="rounded-md px-4 py-2 text-sm font-medium text-gray-400 hover:text-white hover:bg-gray-700 transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={busy || !responseText.trim()}
                className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-500 transition-colors disabled:opacity-50"
              >
                {busy ? 'Submitting…' : 'Submit Response'}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  )
}

export default NotificationResponseDialog
