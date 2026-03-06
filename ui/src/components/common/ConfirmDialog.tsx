/**
 * ConfirmDialog — reusable confirmation modal.
 *
 * Renders a centered dialog with a title, description, and confirm/cancel buttons.
 * The confirm button can be styled as 'danger' (red) for destructive actions.
 */

import { useEffect, useRef } from 'react'
import { X } from 'lucide-react'

export interface ConfirmDialogProps {
  /** Whether the dialog is currently visible */
  open: boolean
  /** Dialog title */
  title: string
  /** Descriptive text body */
  description?: string
  /** Label for the confirm button (default: "Confirm") */
  confirmLabel?: string
  /** Label for the cancel button (default: "Cancel") */
  cancelLabel?: string
  /** 'danger' = red confirm button; 'primary' = blue (default) */
  variant?: 'danger' | 'primary'
  /** Whether the confirm action is in progress */
  loading?: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'primary',
  loading = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null)

  // Focus the cancel button when dialog opens (safe default)
  useEffect(() => {
    if (open) {
      cancelRef.current?.focus()
    }
  }, [open])

  // Close on Escape
  useEffect(() => {
    if (!open) return
    function handleKey(e: KeyboardEvent) {
      if (e.key === 'Escape') onCancel()
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [open, onCancel])

  if (!open) return null

  const confirmClasses =
    variant === 'danger'
      ? 'bg-red-600 text-white hover:bg-red-700 focus:ring-red-500'
      : 'bg-primary-600 text-white hover:bg-primary-700 focus:ring-primary-500'

  return (
    /* Backdrop */
    <div aria-hidden={!open} className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Overlay */}
      <div className="absolute inset-0 bg-black/50" aria-hidden="true" onClick={onCancel} />

      {/* Dialog panel */}
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-dialog-title"
        aria-describedby={description ? 'confirm-dialog-desc' : undefined}
        className="relative w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        {/* Close button */}
        <button
          type="button"
          aria-label="Close dialog"
          onClick={onCancel}
          className="absolute right-4 top-4 rounded-md p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
        >
          <X size={16} />
        </button>

        {/* Title */}
        <h2
          id="confirm-dialog-title"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          {title}
        </h2>

        {/* Description */}
        {description && (
          <p id="confirm-dialog-desc" className="mt-2 text-sm text-gray-500 dark:text-gray-400">
            {description}
          </p>
        )}

        {/* Actions */}
        <div className="mt-5 flex justify-end gap-3">
          <button
            ref={cancelRef}
            type="button"
            onClick={onCancel}
            disabled={loading}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
          >
            {cancelLabel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={loading}
            className={[
              'rounded-md px-4 py-2 text-sm font-medium focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 transition-colors',
              confirmClasses,
            ].join(' ')}
          >
            {loading ? 'Processing…' : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}

export default ConfirmDialog
