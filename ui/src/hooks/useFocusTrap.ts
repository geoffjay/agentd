/**
 * useFocusTrap — trap keyboard focus inside a container element.
 *
 * When active, Tab and Shift+Tab cycle only through focusable elements
 * within the container. Escape key calls the optional `onEscape` callback.
 *
 * Usage:
 *   const ref = useFocusTrap<HTMLDivElement>({ active: isOpen, onEscape: close })
 *   return <div ref={ref}>...</div>
 */

import { useCallback, useEffect, useRef } from 'react'
import { getFocusableElements } from '@/utils/a11y'

export interface UseFocusTrapOptions {
  /** Whether the focus trap is currently active */
  active?: boolean
  /** Called when the user presses Escape inside the trap */
  onEscape?: () => void
  /**
   * If true, focus is moved to the first focusable element inside the
   * container when the trap becomes active. Defaults to true.
   */
  autoFocus?: boolean
  /**
   * If true, focus is restored to the element that had focus before the
   * trap became active when the trap is deactivated. Defaults to true.
   */
  restoreFocus?: boolean
}

export function useFocusTrap<T extends HTMLElement = HTMLElement>({
  active = true,
  onEscape,
  autoFocus = true,
  restoreFocus = true,
}: UseFocusTrapOptions = {}): React.RefObject<T | null> {
  const containerRef = useRef<T | null>(null)
  const previousFocusRef = useRef<HTMLElement | null>(null)

  // Store previously focused element when trap becomes active
  useEffect(() => {
    if (active) {
      previousFocusRef.current = document.activeElement as HTMLElement | null
    } else if (restoreFocus && previousFocusRef.current) {
      previousFocusRef.current.focus()
      previousFocusRef.current = null
    }
  }, [active, restoreFocus])

  // Auto-focus first element on activation
  useEffect(() => {
    if (!active || !autoFocus) return
    const container = containerRef.current
    if (!container) return

    // Defer to next tick to let the element fully render
    const timer = setTimeout(() => {
      const focusable = getFocusableElements(container)
      if (focusable.length > 0) {
        focusable[0].focus()
      } else {
        container.setAttribute('tabindex', '-1')
        container.focus()
      }
    }, 0)

    return () => clearTimeout(timer)
  }, [active, autoFocus])

  // Keyboard handler — trap Tab and handle Escape
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (!active) return
      const container = containerRef.current
      if (!container) return

      if (event.key === 'Escape') {
        event.stopPropagation()
        onEscape?.()
        return
      }

      if (event.key !== 'Tab') return

      const focusable = getFocusableElements(container)
      if (focusable.length === 0) {
        event.preventDefault()
        return
      }

      const first = focusable[0]
      const last = focusable[focusable.length - 1]

      if (event.shiftKey) {
        // Shift+Tab: wrap from first to last
        if (document.activeElement === first) {
          event.preventDefault()
          last.focus()
        }
      } else {
        // Tab: wrap from last to first
        if (document.activeElement === last) {
          event.preventDefault()
          first.focus()
        }
      }
    },
    [active, onEscape],
  )

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

  return containerRef
}
