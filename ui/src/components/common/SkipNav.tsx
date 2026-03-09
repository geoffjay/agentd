/**
 * SkipNav — visually hidden "Skip to main content" link.
 *
 * Becomes visible on focus, allowing keyboard users to bypass the
 * repeated navigation and jump straight to the main content area.
 *
 * The target should be an element with `id="main-content"` (which already
 * exists on the <main> element in ContentArea.tsx).
 *
 * Place this as the very first child of <body> / root layout.
 *
 * WCAG 2.4.1 — Bypass Blocks
 */

export interface SkipNavProps {
  /** ID of the main content element to skip to. Default: "main-content" */
  contentId?: string
  /** Label text. Default: "Skip to main content" */
  label?: string
}

export function SkipNav({
  contentId = 'main-content',
  label = 'Skip to main content',
}: SkipNavProps) {
  return (
    <a
      href={`#${contentId}`}
      className={[
        // Normally hidden off-screen
        'fixed top-2 left-2 z-[9999]',
        'translate-y-[-200%]',
        // Revealed on focus with a smooth transition
        'focus:translate-y-0',
        'transition-transform duration-150',
        // Visible styling
        'rounded-md bg-primary-600 px-4 py-2 text-sm font-semibold text-white shadow-lg',
        'focus:outline-none focus:ring-2 focus:ring-white focus:ring-offset-2 focus:ring-offset-primary-600',
      ].join(' ')}
    >
      {label}
    </a>
  )
}

export default SkipNav
