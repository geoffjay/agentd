/**
 * HighlightedCode — reusable syntax-highlighted code block.
 *
 * Wraps react-shiki with project-consistent styling and sensible defaults.
 * Automatically follows the app's light/dark theme by observing the `dark`
 * class on <html>.
 */

import { useState, useEffect } from 'react'
import ShikiHighlighter from 'react-shiki'

// ---------------------------------------------------------------------------
// Hook — tracks whether the app is in dark mode
// ---------------------------------------------------------------------------

function useIsDark(): boolean {
  const [isDark, setIsDark] = useState(
    () => typeof document !== 'undefined' && document.documentElement.classList.contains('dark'),
  )

  useEffect(() => {
    const el = document.documentElement
    const observer = new MutationObserver(() => {
      setIsDark(el.classList.contains('dark'))
    })
    observer.observe(el, { attributes: true, attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])

  return isDark
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface HighlightedCodeProps {
  /** The text/code to highlight. Empty or undefined renders a placeholder. */
  code: string | undefined | null
  /** Shiki language identifier. Defaults to 'markdown'. */
  language?: string
  /** CSS max-height for the scrollable container. Defaults to '20rem'. */
  maxHeight?: string
  /** Additional CSS classes applied to the outer container. */
  className?: string
}

export function HighlightedCode({
  code,
  language = 'markdown',
  maxHeight = '20rem',
  className = '',
}: HighlightedCodeProps) {
  const isDark = useIsDark()

  if (!code || code.trim().length === 0) {
    return (
      <span className="text-xs italic text-gray-400 dark:text-gray-600">No content</span>
    )
  }

  return (
    <div
      className={['overflow-auto rounded', className].join(' ').trim()}
      style={{ maxHeight }}
    >
      <ShikiHighlighter
        language={language}
        theme={isDark ? 'github-dark' : 'github-light'}
        addDefaultStyles={false}
        style={{
          margin: 0,
          padding: '0.5rem',
          fontSize: '0.75rem',
          lineHeight: '1.5',
          fontFamily: "'JetBrains Mono', 'Fira Code', Consolas, 'Courier New', monospace",
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
          overflowX: 'auto',
        }}
      >
        {code}
      </ShikiHighlighter>
    </div>
  )
}

export default HighlightedCode
