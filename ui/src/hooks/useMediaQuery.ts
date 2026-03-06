/**
 * useMediaQuery — reactive hook for CSS media query matching.
 *
 * Re-renders the component when the query result changes (e.g., on resize).
 *
 * Usage:
 *   const isMobile = useMediaQuery('(max-width: 639px)')
 *   const prefersReducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)')
 *
 * Pre-built breakpoint hooks are also exported for convenience:
 *   const isMobile = useIsMobile()     // < 640px
 *   const isTablet = useIsTablet()     // 640px – 1023px
 *   const isDesktop = useIsDesktop()   // ≥ 1024px
 */

import { useEffect, useState } from 'react'

export function useMediaQuery(query: string): boolean {
  // SSR-safe initialisation: assume false on the server
  const [matches, setMatches] = useState<boolean>(() => {
    if (typeof window === 'undefined') return false
    return window.matchMedia(query).matches
  })

  useEffect(() => {
    if (typeof window === 'undefined') return

    const mql = window.matchMedia(query)
    const onChange = (e: MediaQueryListEvent) => setMatches(e.matches)

    // Set the current value
    setMatches(mql.matches)

    // Modern browsers support addEventListener; older ones use addListener
    if (typeof mql.addEventListener === 'function') {
      mql.addEventListener('change', onChange)
      return () => mql.removeEventListener('change', onChange)
    } else {
      // Legacy fallback (Safari < 14)
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      mql.addListener(onChange)
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      return () => mql.removeListener(onChange)
    }
  }, [query])

  return matches
}

// ---------------------------------------------------------------------------
// Convenience breakpoint hooks (Tailwind-aligned)
// ---------------------------------------------------------------------------

/** True when viewport width < 640px (Tailwind `sm` breakpoint) */
export const useIsMobile = (): boolean => useMediaQuery('(max-width: 639px)')

/** True when 640px ≤ viewport < 1024px (Tailwind `md`–`lg` range) */
export const useIsTablet = (): boolean =>
  useMediaQuery('(min-width: 640px) and (max-width: 1023px)')

/** True when viewport ≥ 1024px (Tailwind `lg` breakpoint) */
export const useIsDesktop = (): boolean => useMediaQuery('(min-width: 1024px)')

/** True when viewport ≥ 1280px (Tailwind `xl` breakpoint) */
export const useIsWide = (): boolean => useMediaQuery('(min-width: 1280px)')

/** True when the user prefers reduced motion */
export const useReducedMotion = (): boolean =>
  useMediaQuery('(prefers-reduced-motion: reduce)')
