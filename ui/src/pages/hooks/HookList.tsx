/**
 * HookList — hooks management page.
 *
 * Renders a "coming soon" placeholder while the hook service (port 17002)
 * is being implemented. The component structure is ready for real data
 * once the service is available — replace the HookPlaceholder with a
 * proper list table and connect the useHooks() hook.
 */

import { Zap } from 'lucide-react'
import { HookPlaceholder } from '@/components/hooks/HookPlaceholder'
import { useHooks } from '@/hooks/useHooks'

export function HookList() {
  // Stub hook — returns empty data until the hook service is implemented.
  // Replace with real data-fetching logic once the service is available.
  const { hooks: _hooks, loading: _loading } = useHooks()

  return (
    <div id="main-content" className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary-100 dark:bg-primary-900/30">
            <Zap size={22} className="text-primary-600 dark:text-primary-400" />
          </div>
          <div>
            <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Hooks</h1>
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Monitor git and system hook events
            </p>
          </div>
        </div>
      </div>

      {/* Coming-soon placeholder / planned feature preview */}
      <HookPlaceholder />
    </div>
  )
}

export default HookList
