/**
 * a11y.ts — Accessibility utility functions.
 *
 * Helpers for:
 * - Keyboard event handling (Enter/Space activation, Escape/Arrow nav)
 * - Focus management (focusable element queries, focus restoration)
 * - Unique ID generation for aria-describedby / htmlFor associations
 * - Reduced-motion detection
 */

// ---------------------------------------------------------------------------
// Focusable element selectors
// ---------------------------------------------------------------------------

/** CSS selector that matches all interactive/focusable elements */
export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
  'details > summary',
].join(', ')

/**
 * Returns all focusable elements within a container, in DOM order.
 * Excludes elements that are hidden (display:none, visibility:hidden).
 */
export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  const elements = Array.from(
    container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  )
  return elements.filter((el) => {
    const style = getComputedStyle(el)
    return style.display !== 'none' && style.visibility !== 'hidden'
  })
}

/**
 * Move focus to the first focusable element inside a container.
 * Falls back to focusing the container itself if nothing is focusable.
 */
export function focusFirst(container: HTMLElement): void {
  const focusable = getFocusableElements(container)
  if (focusable.length > 0) {
    focusable[0].focus()
  } else {
    container.setAttribute('tabindex', '-1')
    container.focus()
  }
}

/**
 * Move focus to the last focusable element inside a container.
 */
export function focusLast(container: HTMLElement): void {
  const focusable = getFocusableElements(container)
  if (focusable.length > 0) {
    focusable[focusable.length - 1].focus()
  }
}

// ---------------------------------------------------------------------------
// Keyboard event helpers
// ---------------------------------------------------------------------------

/** Returns true if the keyboard event is Enter or Space */
export function isActivationKey(event: KeyboardEvent | React.KeyboardEvent): boolean {
  return event.key === 'Enter' || event.key === ' '
}

/** Returns true if the keyboard event is Escape */
export function isEscapeKey(event: KeyboardEvent | React.KeyboardEvent): boolean {
  return event.key === 'Escape'
}

/** Returns true if the keyboard event is an arrow key */
export function isArrowKey(event: KeyboardEvent | React.KeyboardEvent): boolean {
  return ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key)
}

/**
 * Returns an onKeyDown handler that calls `onClick` when Enter or Space
 * is pressed. Use on non-button elements with `role="button"`.
 */
export function onKeyActivate(
  onClick: (event: React.KeyboardEvent) => void,
): (event: React.KeyboardEvent) => void {
  return (event: React.KeyboardEvent) => {
    if (isActivationKey(event)) {
      event.preventDefault()
      onClick(event)
    }
  }
}

// ---------------------------------------------------------------------------
// Unique ID generation
// ---------------------------------------------------------------------------

let _counter = 0

/**
 * Generate a unique ID string with an optional prefix.
 * Useful for linking labels to inputs via htmlFor/id or aria-describedby.
 *
 * @example
 *   const id = generateId('email-field') // → "email-field-1"
 */
export function generateId(prefix = 'agentd'): string {
  return `${prefix}-${++_counter}`
}

/** Reset the counter (useful in tests to get predictable IDs) */
export function resetIdCounter(): void {
  _counter = 0
}

// ---------------------------------------------------------------------------
// Reduced motion detection
// ---------------------------------------------------------------------------

/**
 * Returns true if the user has requested reduced motion via the
 * `prefers-reduced-motion: reduce` media query.
 *
 * Use this to conditionally disable animations:
 *   if (!prefersReducedMotion()) startAnimation()
 */
export function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches
  } catch {
    return false
  }
}

// ---------------------------------------------------------------------------
// Announce to screen readers
// ---------------------------------------------------------------------------

/**
 * Programmatically announce a message to screen readers using a live region.
 *
 * Creates a visually-hidden live region, inserts the message, then removes
 * it after a short delay so the DOM stays clean.
 *
 * @param message Text to announce
 * @param politeness 'polite' (default) or 'assertive'
 */
export function announce(
  message: string,
  politeness: 'polite' | 'assertive' = 'polite',
): void {
  const el = document.createElement('div')
  el.setAttribute('aria-live', politeness)
  el.setAttribute('aria-atomic', 'true')
  el.setAttribute('role', politeness === 'assertive' ? 'alert' : 'status')

  // Visually hidden but readable by screen readers
  Object.assign(el.style, {
    position: 'absolute',
    width: '1px',
    height: '1px',
    padding: '0',
    margin: '-1px',
    overflow: 'hidden',
    clip: 'rect(0,0,0,0)',
    whiteSpace: 'nowrap',
    border: '0',
  })

  document.body.appendChild(el)

  // Small delay ensures screen readers pick up the dynamic content change
  requestAnimationFrame(() => {
    el.textContent = message
    setTimeout(() => {
      document.body.removeChild(el)
    }, 1000)
  })
}
