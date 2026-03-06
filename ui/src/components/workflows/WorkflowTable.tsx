/**
 * WorkflowTable — paginated table of workflow configurations.
 *
 * Columns: name, agent, source summary, poll interval, enabled toggle,
 *          dispatch count (from history), actions (edit, delete).
 */

import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Edit2, Eye, Trash2 } from 'lucide-react'
import { AgentStatusBadge } from '@/components/agents/AgentStatusBadge'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { ListItemSkeleton } from '@/components/common/LoadingSkeleton'
import type { Agent, Workflow } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface WorkflowTableProps {
  workflows: Workflow[]
  agents: Agent[]
  loading: boolean
  onEdit: (workflow: Workflow) => void
  onDelete: (id: string) => Promise<void>
  onToggleEnabled: (id: string, enabled: boolean) => Promise<void>
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sourceLabel(workflow: Workflow): string {
  const src = workflow.source_config
  if (src.type === 'github_issues') {
    const parts = [`${src.owner}/${src.repo}`]
    if (src.labels.length > 0) parts.push(`#${src.labels[0]}`)
    if (src.labels.length > 1) parts.push(`+${src.labels.length - 1}`)
    return `GitHub: ${parts.join(' ')}`
  }
  return src.type
}

function formatInterval(secs: number): string {
  if (secs < 60) return `${secs}s`
  const mins = Math.round(secs / 60)
  if (mins < 60) return `${mins}m`
  return `${Math.round(mins / 60)}h`
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function EmptyState() {
  return (
    <tr>
      <td colSpan={7} className="py-12 text-center">
        <p className="text-sm text-gray-500 dark:text-gray-400">
          No workflows configured yet.
        </p>
        <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
          Create a workflow to start dispatching tasks automatically.
        </p>
      </td>
    </tr>
  )
}

// ---------------------------------------------------------------------------
// Toggle switch
// ---------------------------------------------------------------------------

function ToggleSwitch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
  label: string
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={[
        'relative inline-flex h-5 w-9 items-center rounded-full transition-colors',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        checked ? 'bg-primary-500' : 'bg-gray-200 dark:bg-gray-700',
      ].join(' ')}
    >
      <span
        className={[
          'inline-block h-3.5 w-3.5 rounded-full bg-white shadow transition-transform',
          checked ? 'translate-x-4' : 'translate-x-0.5',
        ].join(' ')}
      />
    </button>
  )
}

// ---------------------------------------------------------------------------
// WorkflowTable
// ---------------------------------------------------------------------------

export function WorkflowTable({
  workflows,
  agents,
  loading,
  onEdit,
  onDelete,
  onToggleEnabled,
}: WorkflowTableProps) {
  const navigate = useNavigate()
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const [togglingId, setTogglingId] = useState<string | null>(null)

  const agentMap = new Map(agents.map((a) => [a.id, a]))

  async function handleToggle(id: string, enabled: boolean) {
    setTogglingId(id)
    try {
      await onToggleEnabled(id, enabled)
    } finally {
      setTogglingId(null)
    }
  }

  async function handleDelete() {
    if (!deletingId) return
    await onDelete(deletingId)
    setDeletingId(null)
  }

  return (
    <>
      <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-700">
        <table className="min-w-full text-sm">
          <thead className="bg-gray-50 dark:bg-gray-800">
            <tr>
              {['Workflow', 'Agent', 'Source', 'Interval', 'Enabled', 'Updated', ''].map((h) => (
                <th
                  key={h}
                  className="py-3 px-4 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide whitespace-nowrap"
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {loading ? (
              Array.from({ length: 3 }).map((_, i) => (
                <tr key={i}>
                  <td colSpan={7} className="p-2">
                    <ListItemSkeleton />
                  </td>
                </tr>
              ))
            ) : workflows.length === 0 ? (
              <EmptyState />
            ) : (
              workflows.map((wf) => {
                const agent = agentMap.get(wf.agent_id)
                return (
                  <tr
                    key={wf.id}
                    className="hover:bg-gray-50 dark:hover:bg-gray-800/40 transition-colors"
                  >
                    {/* Name */}
                    <td className="py-3 px-4">
                      <button
                        type="button"
                        onClick={() => navigate(`/workflows/${wf.id}`)}
                        className="font-medium text-primary-600 dark:text-primary-400 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 rounded"
                      >
                        {wf.name}
                      </button>
                    </td>

                    {/* Agent */}
                    <td className="py-3 px-4">
                      {agent ? (
                        <div className="flex items-center gap-2">
                          <AgentStatusBadge status={agent.status} variant="dot" />
                          <span className="text-gray-700 dark:text-gray-300 text-xs">
                            {agent.name}
                          </span>
                        </div>
                      ) : (
                        <span className="text-xs text-gray-400 dark:text-gray-500">
                          Unknown agent
                        </span>
                      )}
                    </td>

                    {/* Source */}
                    <td className="py-3 px-4 text-xs text-gray-500 dark:text-gray-400">
                      {sourceLabel(wf)}
                    </td>

                    {/* Poll interval */}
                    <td className="py-3 px-4 text-xs text-gray-500 dark:text-gray-400">
                      {formatInterval(wf.poll_interval_secs)}
                    </td>

                    {/* Enabled toggle */}
                    <td className="py-3 px-4">
                      <ToggleSwitch
                        checked={wf.enabled}
                        onChange={(v) => handleToggle(wf.id, v)}
                        disabled={togglingId === wf.id}
                        label={`${wf.enabled ? 'Disable' : 'Enable'} ${wf.name}`}
                      />
                    </td>

                    {/* Updated */}
                    <td className="py-3 px-4 text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
                      {new Date(wf.updated_at).toLocaleDateString()}
                    </td>

                    {/* Actions */}
                    <td className="py-3 px-4">
                      <div className="flex items-center gap-1">
                        <button
                          type="button"
                          onClick={() => navigate(`/workflows/${wf.id}`)}
                          className="rounded p-1 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
                          aria-label={`View ${wf.name}`}
                        >
                          <Eye size={15} />
                        </button>
                        <button
                          type="button"
                          onClick={() => onEdit(wf)}
                          className="rounded p-1 text-gray-400 hover:text-primary-600 dark:hover:text-primary-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
                          aria-label={`Edit ${wf.name}`}
                        >
                          <Edit2 size={15} />
                        </button>
                        <button
                          type="button"
                          onClick={() => setDeletingId(wf.id)}
                          className="rounded p-1 text-gray-400 hover:text-red-600 dark:hover:text-red-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-500"
                          aria-label={`Delete ${wf.name}`}
                        >
                          <Trash2 size={15} />
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              })
            )}
          </tbody>
        </table>
      </div>

      <ConfirmDialog
        open={deletingId !== null}
        title="Delete workflow"
        description="This will permanently delete the workflow and stop all scheduled polling. Existing dispatch records will be removed."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDelete}
        onCancel={() => setDeletingId(null)}
      />
    </>
  )
}

export default WorkflowTable
