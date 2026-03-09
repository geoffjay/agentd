/**
 * ContentArea — scrollable main content wrapper.
 *
 * Offsets for the fixed header (h-16 = 64px) and sidebar width.
 * Transitions smoothly when the sidebar expands/collapses.
 */

import type { ReactNode } from 'react'
import { useLayout } from './context'

interface ContentAreaProps {
  children: ReactNode
}

export function ContentArea({ children }: ContentAreaProps) {
  const { sidebarOpen } = useLayout()

  return (
    <main
      id="main-content"
      className={[
        'min-h-[calc(100vh-4rem)]',
        'mt-16', // offset for fixed header (h-16)
        'overflow-y-auto',
        'transition-all duration-300 ease-in-out',
        // On large screens, shift right by sidebar width
        sidebarOpen ? 'lg:ml-60' : 'lg:ml-16',
      ].join(' ')}
    >
      {/* Inner wrapper: responsive padding + max-width centering */}
      <div className="mx-auto max-w-screen-2xl p-4 md:p-6 lg:p-8">{children}</div>
    </main>
  )
}

export default ContentArea
