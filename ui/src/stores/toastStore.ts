/**
 * toastStore — lightweight pub/sub toast store (no external state library).
 *
 * Used by:
 * - useToast()   hook — to dispatch toasts from anywhere in the app
 * - ToastContainer — to subscribe and render the live toast list
 *
 * Design: a singleton module-level store with a Set of listeners so that
 * multiple consumers can subscribe independently.
 */

const uuidv4 = (): string => crypto.randomUUID()

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface ToastAction {
  label: string
  onClick: () => void
}

export interface Toast {
  id: string
  type: ToastType
  title: string
  message?: string
  /** Duration in ms before auto-dismiss; 0 = never auto-dismiss */
  duration: number
  action?: ToastAction
  createdAt: number
}

export type AddToastOptions = Omit<Toast, 'id' | 'createdAt'>

// ---------------------------------------------------------------------------
// Default durations
// ---------------------------------------------------------------------------

const DEFAULT_DURATION: Record<ToastType, number> = {
  success: 5_000,
  info: 5_000,
  warning: 5_000,
  error: 8_000,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

type Listener = (toasts: Toast[]) => void

let toasts: Toast[] = []
const listeners = new Set<Listener>()

function notify(): void {
  const snapshot = [...toasts]
  listeners.forEach((fn) => fn(snapshot))
}

export const toastStore = {
  /** Get the current snapshot (useful for initial render) */
  getToasts(): Toast[] {
    return [...toasts]
  },

  /** Subscribe to toast list changes. Returns an unsubscribe function. */
  subscribe(listener: Listener): () => void {
    listeners.add(listener)
    return () => listeners.delete(listener)
  },

  /** Add a new toast and return its ID */
  add(options: AddToastOptions): string {
    const id = uuidv4()
    const toast: Toast = {
      ...options,
      duration: options.duration ?? DEFAULT_DURATION[options.type],
      id,
      createdAt: Date.now(),
    }
    toasts = [...toasts, toast]
    notify()
    return id
  },

  /** Convenience: success toast */
  success(title: string, options?: Partial<Omit<AddToastOptions, 'type' | 'title'>>): string {
    return toastStore.add({ type: 'success', title, duration: DEFAULT_DURATION.success, ...options })
  },

  /** Convenience: error toast */
  error(title: string, options?: Partial<Omit<AddToastOptions, 'type' | 'title'>>): string {
    return toastStore.add({ type: 'error', title, duration: DEFAULT_DURATION.error, ...options })
  },

  /** Convenience: warning toast */
  warning(title: string, options?: Partial<Omit<AddToastOptions, 'type' | 'title'>>): string {
    return toastStore.add({ type: 'warning', title, duration: DEFAULT_DURATION.warning, ...options })
  },

  /** Convenience: info toast */
  info(title: string, options?: Partial<Omit<AddToastOptions, 'type' | 'title'>>): string {
    return toastStore.add({ type: 'info', title, duration: DEFAULT_DURATION.info, ...options })
  },

  /** Remove a specific toast by ID */
  dismiss(id: string): void {
    toasts = toasts.filter((t) => t.id !== id)
    notify()
  },

  /** Remove all toasts */
  clear(): void {
    toasts = []
    notify()
  },
}
