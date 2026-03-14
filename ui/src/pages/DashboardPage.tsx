/**
 * DashboardPage — landing page with service health, agent summary,
 * notification summary, activity timeline, and stub sections.
 */

import { useMemo } from 'react'
import { BarChart2, RefreshCw, Webhook } from 'lucide-react'
import {
  ServiceHealthCard,
  ServiceHealthCardSkeleton,
} from '@/components/dashboard/ServiceHealthCard'
import { AgentSummary } from '@/components/dashboard/AgentSummary'
import { NotificationSummary } from '@/components/dashboard/NotificationSummary'
import { ActivityTimeline } from '@/components/dashboard/ActivityTimeline'
import type { ActivityEvent } from '@/components/dashboard/ActivityTimeline'
import { useServiceHealth } from '@/hooks/useServiceHealth'
import { useAgentSummary } from '@/hooks/useAgentSummary'
import { useNotificationSummary } from '@/hooks/useNotificationSummary'

// ---------------------------------------------------------------------------
// Stub "Coming Soon" card
// ---------------------------------------------------------------------------

interface ComingSoonCardProps {
  title: string
  icon: React.ReactNode
}

function ComingSoonCard({ title, icon }: ComingSoonCardProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-gray-300 bg-white p-8 text-center dark:border-gray-600 dark:bg-gray-800">
      <div className="flex h-12 w-12 items-center justify-center rounded-full bg-gray-100 dark:bg-gray-700">
        {icon}
      </div>
      <div>
        <p className="font-medium text-gray-700 dark:text-gray-300">{title}</p>
        <p className="mt-1 text-sm text-gray-400 dark:text-gray-500">Coming Soon</p>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Dashboard page
// ---------------------------------------------------------------------------

export function DashboardPage() {
  const { services, loading: healthLoading, initializing: healthInit, refresh } = useServiceHealth()
  const agentSummary = useAgentSummary()
  const notifSummary = useNotificationSummary()

  // Build a synthetic activity feed from available data
  const activityEvents: ActivityEvent[] = useMemo(() => {
    const events: ActivityEvent[] = []

    // Derive events from recently-updated agents
    for (const agent of agentSummary.recentAgents) {
      events.push({
        id: `agent-${agent.id}`,
        type: 'agent',
        description: `Agent "${agent.name}" is ${agent.status.toLowerCase()}`,
        timestamp: new Date(agent.updated_at),
      })
    }

    // Sort by timestamp descending
    return events.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime()).slice(0, 10)
  }, [agentSummary.recentAgents])

  return (
    <div className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Dashboard</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
            System overview and service health
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={healthLoading}
          aria-label="Refresh service health"
          className="flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-700 hover:bg-gray-50 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
        >
          <RefreshCw size={14} className={healthLoading ? 'animate-spin' : ''} />
          Refresh
        </button>
      </div>

      {/* Service health cards */}
      <section aria-labelledby="service-health-heading">
        <h2
          id="service-health-heading"
          className="mb-3 text-sm font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500"
        >
          Service Health
        </h2>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {healthInit
            ? Array.from({ length: 4 }).map((_, i) => <ServiceHealthCardSkeleton key={i} />)
            : services.map((svc) => <ServiceHealthCard key={svc.key} service={svc} />)}
        </div>
      </section>

      {/* Main grid: agents + notifications */}
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <AgentSummary {...agentSummary} />
        <NotificationSummary {...notifSummary} />
      </div>

      {/* Activity timeline */}
      <ActivityTimeline
        events={activityEvents}
        loading={agentSummary.loading}
        error={agentSummary.error}
      />

      {/* Stub sections */}
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <ComingSoonCard
          title="Monitoring"
          icon={<BarChart2 size={24} className="text-gray-400" />}
        />
        <ComingSoonCard title="Hooks" icon={<Webhook size={24} className="text-gray-400" />} />
      </div>
    </div>
  )
}

export default DashboardPage
