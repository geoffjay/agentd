/**
 * ToastContainer — subscribes to toastStore and renders all active toasts.
 *
 * Positioned fixed top-right. Should be mounted once inside AppShell.
 * Provides an aria-live region so screen readers announce new toasts.
 */

import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { toastStore } from '@/stores/toastStore'
import type { Toast as ToastData } from '@/stores/toastStore'
import { Toast } from './Toast'

export function ToastContainer() {
  const [toasts, setToasts] = useState<ToastData[]>(() => toastStore.getToasts())

  useEffect(() => {
    return toastStore.subscribe(setToasts)
  }, [])

  const container = (
    <div
      aria-label="Notifications"
      className="fixed top-20 right-4 z-50 flex flex-col gap-2 pointer-events-none"
    >
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto">
          <Toast
            toast={toast}
            onDismiss={toastStore.dismiss}
          />
        </div>
      ))}
    </div>
  )

  return createPortal(container, document.body)
}

export default ToastContainer
