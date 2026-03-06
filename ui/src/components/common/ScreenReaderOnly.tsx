/**
 * ScreenReaderOnly — visually hidden content accessible only to screen readers.
 *
 * Uses the standard "sr-only" CSS technique: element is 1×1px with overflow
 * clipped, invisible on screen but present in the accessibility tree.
 *
 * Usage:
 *   <button aria-label="Close">
 *     <X size={16} aria-hidden="true" />
 *     <ScreenReaderOnly>Close dialog</ScreenReaderOnly>
 *   </button>
 *
 * Note: Tailwind's `sr-only` utility already implements this pattern. This
 * component is a convenience wrapper for cases where you need it as a
 * React element rather than a className.
 */

import type { ReactNode } from 'react'

export interface ScreenReaderOnlyProps {
  children: ReactNode
  /** HTML element to render; default "span" (inline, safe inside buttons) */
  as?: 'span' | 'div' | 'p' | 'h2' | 'h3'
  className?: string
}

export function ScreenReaderOnly({
  children,
  as: Tag = 'span',
  className = '',
}: ScreenReaderOnlyProps) {
  return (
    <Tag
      className={[
        'absolute w-px h-px p-0 -m-px overflow-hidden whitespace-nowrap',
        '[clip:rect(0,0,0,0)] border-0',
        className,
      ].join(' ')}
    >
      {children}
    </Tag>
  )
}

export default ScreenReaderOnly
