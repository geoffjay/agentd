/**
 * Toast — individual toast notification card.
 *
 * Shows:
 * - Colour-coded left border and icon by type (success/error/warning/info)
 * - Title and optional message
 * - Optional action button
 * - Manual dismiss (X) button
 * - Auto-dismiss progress bar when duration > 0
 */

import { useEffect, useRef, useState } from 'react'
import { AlertCircle, AlertTriangle, CheckCircle2, Info, X } from 'lucide-react'
import type { Toast as ToastData } from '@/stores/toastStore'

// ---------------------------------------------------------------------------
// Styling maps
// ---------------------------------------------------------------------------

const BORDER: Record<string, string> = {
  success: 'border-l-green-500',
  error: 'border-l-red-500',
  warning: 'border-l-yellow-500',
  info: 'border-l-blue-500',
}

const ICON_CLASS: Record<string, string> = {
  success: 'text-green-400',
  error: 'text-red-400',
  warning: 'text-yellow-400',
  info: 'text-blue-400',
}

const PROGRESS_CLASS: Record<string, string> = {
  success: 'bg-green-500',
  error: 'bg-red-500',
  warning: 'bg-yellow-500',
  info: 'bg-blue-500',
}

function ToastIcon({ type }: { type: string }) {
  const cls = ['shrink-0', ICON_CLASS[type] ?? 'text-gray-400'].join(' ')
  switch (type) {
    case 'success':
      return <CheckCircle2 size={18} className={cls} aria-hidden="true" />
    case 'error':
      return <AlertCircle size={18} className={cls} aria-hidden="true" />
    case 'warning':
      return <AlertTriangle size={18} className={cls} aria-hidden="true" />
    default:
      return <Info size={18} className={cls} aria-hidden="true" />
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ToastProps {
  toast: ToastData
  onDismiss: (id: string) => void
}

export function Toast({ toast, onDismiss }: ToastProps) {
  const { id, type, title, message, duration, action } = toast
  const [progress, setProgress] = useState(100)
  const startRef = useRef(Date.now())
  const frameRef = useRef<number | undefined>(undefined)

  // Auto-dismiss with animated progress bar
  useEffect(() => {
    if (!duration) return

    const tick = () => {
      const elapsed = Date.now() - startRef.current
      const remaining = Math.max(0, 100 - (elapsed / duration) * 100)
      setProgress(remaining)
      if (remaining > 0) {
        frameRef.current = requestAnimationFrame(tick)
      } else {
        onDismiss(id)
      }
    }

    frameRef.current = requestAnimationFrame(tick)
    return () => {
      if (frameRef.current) cancelAnimationFrame(frameRef.current)
    }
  }, [id, duration, onDismiss])

  return (
    <div
      role="alert"
      aria-live={type === 'error' ? 'assertive' : 'polite'}
      aria-atomic="true"
      className={[
        'relative overflow-hidden rounded-lg border border-gray-700 bg-gray-800 shadow-xl',
        'border-l-4',
        BORDER[type] ?? 'border-l-gray-500',
      ].join(' ')}
    >
      <div className="flex items-start gap-3 p-4">
        <ToastIcon type={type} />

        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-white">{title}</p>
          {message && (
            <p className="mt-0.5 text-xs text-gray-400 break-words">{message}</p>
          )}
          {action && (
            <button
              type="button"
              onClick={() => {
                action.onClick()
                onDismiss(id)
              }}
              className="mt-2 text-xs font-medium text-primary-400 hover:text-primary-300 transition-colors"
            >
              {action.label}
            </button>
          )}
        </div>

        <button
          type="button"
          aria-label="Dismiss notification"
          onClick={() => onDismiss(id)}
          className="shrink-0 rounded-md p-0.5 text-gray-400 hover:bg-gray-700 hover:text-white transition-colors"
        >
          <X size={14} />
        </button>
      </div>

      {/* Progress bar */}
      {duration > 0 && (
        <div
          className={['absolute bottom-0 left-0 h-0.5 transition-none', PROGRESS_CLASS[type] ?? 'bg-gray-500'].join(' ')}
          style={{ width: `${progress}%` }}
          aria-hidden="true"
        />
      )}
    </div>
  )
}

export default Toast
