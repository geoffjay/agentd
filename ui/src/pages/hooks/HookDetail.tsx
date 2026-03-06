/**
 * HookDetail — placeholder for the individual hook detail page.
 *
 * This component will display the full configuration and event log for a
 * single hook once the hook service (port 17002) is implemented. For now
 * it renders a minimal placeholder that is ready to be wired up.
 */

import { useParams } from 'react-router-dom'
import { ArrowLeft, Zap } from 'lucide-react'
import { Link } from 'react-router-dom'

export function HookDetail() {
  const { id } = useParams<{ id: string }>()

  return (
    <div id="main-content" className="space-y-6">
      {/* Back nav */}
      <Link
        to="/hooks"
        className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 rounded"
      >
        <ArrowLeft size={16} />
        Back to Hooks
      </Link>

      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary-100 dark:bg-primary-900/30">
          <Zap size={22} className="text-primary-600 dark:text-primary-400" />
        </div>
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">
            Hook {id ?? '—'}
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Hook detail — coming soon
          </p>
        </div>
      </div>

      {/* Placeholder body */}
      <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50/50 p-10 text-center dark:border-gray-700 dark:bg-gray-800/50">
        <p className="text-gray-400 dark:text-gray-500">
          Hook detail view will be available once the hook service is implemented.
        </p>
      </div>
    </div>
  )
}

export default HookDetail
