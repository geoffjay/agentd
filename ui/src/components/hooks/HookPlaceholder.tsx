/**
 * HookPlaceholder — coming-soon placeholder for the hooks management page.
 *
 * Renders a wireframe preview of the planned hooks feature set:
 *   - Service health indicator for the hook service (port 17002)
 *   - Feature cards for git hooks, system hooks, event log, configuration,
 *     and notification triggers
 *   - Link to the GitHub issue tracking implementation progress
 */

import { Activity, Bell, Code2, GitBranch, Info, Settings, Zap } from 'lucide-react'
import { StatusBadge } from '@/components/common/StatusBadge'
import { HOOK_SERVICE_PORT } from '@/hooks/useHooks'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface PlannedFeatureCardProps {
  icon: React.ReactNode
  title: string
  description: string
  items: string[]
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function PlannedFeatureCard({ icon, title, description, items }: PlannedFeatureCardProps) {
  return (
    <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50/50 p-5 dark:border-gray-700 dark:bg-gray-800/50">
      <div className="flex items-center gap-3 mb-3">
        <div className="flex h-9 w-9 items-center justify-center rounded-md bg-primary-100 dark:bg-primary-900/30">
          {icon}
        </div>
        <h3 className="font-medium text-gray-900 dark:text-white">{title}</h3>
      </div>
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-3">{description}</p>
      <ul className="space-y-1">
        {items.map((item) => (
          <li key={item} className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
            <span className="h-1 w-1 rounded-full bg-gray-300 dark:bg-gray-600 flex-shrink-0" />
            {item}
          </li>
        ))}
      </ul>
    </div>
  )
}

function ServiceStatusBanner({ port }: { port: number }) {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-gray-200 bg-white p-4 shadow-sm dark:border-gray-700 dark:bg-gray-800">
      <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-gray-100 dark:bg-gray-700">
        <Activity size={18} className="text-gray-500 dark:text-gray-400" />
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-900 dark:text-white">Hook Service</p>
        <p className="text-xs text-gray-500 dark:text-gray-400">Port {port}</p>
      </div>
      <StatusBadge status="unknown" />
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main export
// ---------------------------------------------------------------------------

export function HookPlaceholder() {
  return (
    <div className="space-y-8">
      {/* Coming Soon header */}
      <div className="text-center py-8">
        <div className="flex justify-center mb-4">
          <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-100 dark:bg-primary-900/30">
            <Zap size={32} className="text-primary-600 dark:text-primary-400" />
          </div>
        </div>
        <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
          Hooks — Coming Soon
        </h2>
        <p className="text-gray-500 dark:text-gray-400">
          The hook service will monitor git hooks and system hooks, automatically creating
          notifications when hook events fire. Configure triggers, view execution logs, and
          connect hook events to your notification workflows.
        </p>
      </div>

      {/* Service health indicator */}
      <div>
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-3">
          Service Status
        </h3>
        <ServiceStatusBanner port={HOOK_SERVICE_PORT} />
        <p className="mt-2 flex items-center gap-1.5 text-xs text-gray-400 dark:text-gray-500">
          <Info size={12} />
          The hook service is not yet implemented. Status will update automatically once the
          service is running.
        </p>
      </div>

      {/* Planned feature preview */}
      <div>
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300 uppercase tracking-wide mb-4">
          Planned Features
        </h3>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <PlannedFeatureCard
            icon={<GitBranch size={18} className="text-primary-600 dark:text-primary-400" />}
            title="Git Hooks"
            description="Monitor git lifecycle events in your repositories."
            items={[
              'pre-commit — validate before commit',
              'post-commit — trigger after commit',
              'pre-push — gate pushes',
              'post-merge — react to merges',
              'post-checkout — respond to checkouts',
            ]}
          />
          <PlannedFeatureCard
            icon={<Code2 size={18} className="text-primary-600 dark:text-primary-400" />}
            title="System Hooks"
            description="React to system-level and process events."
            items={[
              'File system changes',
              'Process start / stop events',
              'Scheduled cron triggers',
              'Custom shell script hooks',
              'Environment variable changes',
            ]}
          />
          <PlannedFeatureCard
            icon={<Activity size={18} className="text-primary-600 dark:text-primary-400" />}
            title="Event Log"
            description="Full audit trail of every hook execution."
            items={[
              'Timestamp and duration per event',
              'Success / failure status',
              'Payload inspection',
              'Error details for failures',
              'Filterable by hook or event type',
            ]}
          />
          <PlannedFeatureCard
            icon={<Settings size={18} className="text-primary-600 dark:text-primary-400" />}
            title="Hook Configuration"
            description="Enable, disable, and tune individual hooks."
            items={[
              'Toggle hooks on / off',
              'Edit hook event bindings',
              'Set per-hook retry policy',
              'Configure timeout thresholds',
              'Import / export hook definitions',
            ]}
          />
          <PlannedFeatureCard
            icon={<Bell size={18} className="text-primary-600 dark:text-primary-400" />}
            title="Notification Triggers"
            description="Automatically create notifications when hooks fire."
            items={[
              'Trigger on success, failure, or both',
              'Customisable message templates',
              'Set notification priority per hook',
              'Route to specific notification channels',
              'Suppress duplicate notifications',
            ]}
          />
        </div>
      </div>

      {/* Track progress link */}
      <div className="rounded-lg border border-blue-100 bg-blue-50 p-4 dark:border-blue-900/40 dark:bg-blue-900/10">
        <p className="text-sm text-blue-700 dark:text-blue-300">
          <span className="font-medium">Track progress:</span>{' '}
          Follow the hook service implementation on the{' '}
          <a
            href="https://github.com/geoffjay/agentd/issues/179"
            target="_blank"
            rel="noopener noreferrer"
            className="underline hover:text-blue-900 dark:hover:text-blue-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 rounded"
          >
            GitHub issue #179
          </a>
          .
        </p>
      </div>
    </div>
  )
}

export default HookPlaceholder
