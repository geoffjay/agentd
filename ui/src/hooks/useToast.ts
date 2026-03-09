/**
 * useToast — convenient hook for dispatching toast notifications.
 *
 * Returns the toastStore API directly so components can call:
 *   const { success, error, warning, info, dismiss } = useToast()
 *
 * Also re-exports mapApiError for consistent API error messaging.
 */

import { toastStore } from '@/stores/toastStore'
import type { ToastAction } from '@/stores/toastStore'
import { ApiError } from '@/types/common'

// ---------------------------------------------------------------------------
// API error → user-friendly message mapping
// ---------------------------------------------------------------------------

/**
 * Map an ApiError status code to a user-friendly message string.
 * Falls back to the raw error message for unmapped codes.
 */
export function mapApiError(err: unknown): string {
  if (err instanceof ApiError) {
    switch (err.status) {
      case 0:
        return 'Service unavailable — check your connection'
      case 400:
        return err.message.startsWith('HTTP ')
          ? 'Invalid request — check your input'
          : err.message
      case 401:
        return 'Unauthorized — please log in'
      case 403:
        return 'Forbidden — you do not have permission'
      case 404:
        return 'Resource not found'
      case 408:
        return 'Request timed out — please try again'
      case 409:
        return 'Conflict — resource already exists'
      case 422:
        return err.message.startsWith('HTTP ')
          ? 'Validation error — check your input'
          : err.message
      case 429:
        return 'Too many requests — please slow down'
      default:
        if (err.status >= 500) {
          return 'Server error — please try again later'
        }
        return err.message
    }
  }
  if (err instanceof Error) return err.message
  return String(err)
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseToastReturn {
  success: (title: string, options?: { message?: string; action?: ToastAction }) => string
  error: (title: string, options?: { message?: string; action?: ToastAction; duration?: number }) => string
  warning: (title: string, options?: { message?: string; action?: ToastAction }) => string
  info: (title: string, options?: { message?: string; action?: ToastAction }) => string
  dismiss: (id: string) => void
  clear: () => void
  /**
   * Show an error toast for an API or network error.
   * Automatically maps status codes to friendly messages.
   */
  apiError: (err: unknown, title?: string) => string
}

export function useToast(): UseToastReturn {
  return {
    success: (title, options) =>
      toastStore.success(title, options),
    error: (title, options) =>
      toastStore.error(title, options),
    warning: (title, options) =>
      toastStore.warning(title, options),
    info: (title, options) =>
      toastStore.info(title, options),
    dismiss: toastStore.dismiss,
    clear: toastStore.clear,
    apiError: (err, title = 'Something went wrong') => {
      const message = mapApiError(err)
      return toastStore.error(title, { message })
    },
  }
}
