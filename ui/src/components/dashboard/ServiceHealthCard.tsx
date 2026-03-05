/**
 * ServiceHealthCard — shows a single service's health status, version, and port.
 */

import { useNavigate } from 'react-router-dom'
import { Activity, Server } from 'lucide-react'
import { StatusBadge } from '@/components/common/StatusBadge'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import type { ServiceHealth } from '@/hooks/useServiceHealth'

const SERVICE_ROUTES: Record<string, string> = {
  orchestrator: '/agents',
  notify: '/notifications',
  ask: '/questions',
}

interface ServiceHealthCardProps {
  service: ServiceHealth
}

export function ServiceHealthCard({ service }: ServiceHealthCardProps) {
  const navigate = useNavigate()

  function handleClick() {
    const route = SERVICE_ROUTES[service.key]
    if (route) navigate(route)
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      handleClick()
    }
  }

  return (
    <div
      role="button"
      tabIndex={0}
      aria-label={`${service.name} service — ${service.status}`}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      className="cursor-pointer rounded-lg border border-gray-200 bg-white p-5 shadow-sm transition-shadow hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 dark:border-gray-700 dark:bg-gray-800"
    >
      {/* Header row */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary-100 dark:bg-primary-900/30">
            <Server size={20} className="text-primary-600 dark:text-primary-400" />
          </div>
          <div>
            <p className="font-semibold text-gray-900 dark:text-white">{service.name}</p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Port {service.port}</p>
          </div>
        </div>
        <StatusBadge status={service.status} />
      </div>

      {/* Version + last checked */}
      <div className="mt-4 flex items-center justify-between text-xs text-gray-500 dark:text-gray-400">
        <span className="flex items-center gap-1">
          <Activity size={12} />
          {service.version ? `v${service.version}` : '—'}
        </span>
        {service.lastChecked && (
          <span>Checked {formatRelativeTime(service.lastChecked)}</span>
        )}
      </div>

      {/* Error message */}
      {service.error && (
        <p className="mt-2 text-xs text-red-500 dark:text-red-400">{service.error}</p>
      )}
    </div>
  )
}

/** Loading placeholder matching the card's dimensions */
export function ServiceHealthCardSkeleton() {
  return <CardSkeleton />
}

/** Format a Date as a short relative time string */
function formatRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime()
  const diffSec = Math.floor(diffMs / 1000)
  if (diffSec < 60) return 'just now'
  const diffMin = Math.floor(diffSec / 60)
  if (diffMin < 60) return `${diffMin}m ago`
  const diffHr = Math.floor(diffMin / 60)
  return `${diffHr}h ago`
}

export default ServiceHealthCard
