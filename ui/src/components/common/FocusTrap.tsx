/**
 * FocusTrap — React component wrapper around the useFocusTrap hook.
 *
 * Traps keyboard focus inside its children when `active` is true.
 * Useful for modals, drawers, and dialogs.
 *
 * Usage:
 *   <FocusTrap active={isOpen} onEscape={close}>
 *     <div role="dialog" aria-modal="true">
 *       ...
 *     </div>
 *   </FocusTrap>
 */

import type { ReactNode } from 'react'
import { useFocusTrap } from '@/hooks/useFocusTrap'

export interface FocusTrapProps {
  children: ReactNode
  /** Whether the trap is currently active. Default: true */
  active?: boolean
  /** Called when Escape is pressed inside the trap */
  onEscape?: () => void
  /** Move focus to first element on activation. Default: true */
  autoFocus?: boolean
  /** Restore focus to the previously focused element when deactivated. Default: true */
  restoreFocus?: boolean
  className?: string
}

export function FocusTrap({
  children,
  active = true,
  onEscape,
  autoFocus = true,
  restoreFocus = true,
  className,
}: FocusTrapProps) {
  const ref = useFocusTrap<HTMLDivElement>({
    active,
    onEscape,
    autoFocus,
    restoreFocus,
  })

  return (
    <div ref={ref} className={className}>
      {children}
    </div>
  )
}

export default FocusTrap
