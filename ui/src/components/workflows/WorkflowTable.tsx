/**
 * WorkflowTable — paginated table of workflow configurations.
 *
 * Uses the common DataTable component with clickable rows that
 * navigate to the workflow detail page.
 *
 * Columns: name, agent, source summary, poll interval, enabled toggle,
 *          updated date, actions (edit, delete).
 */

import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Edit2, Trash2 } from 'lucide-react'
import { AgentStatusBadge } from '@/components/agents/AgentStatusBadge'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { DataTable } from '@/components/common/DataTable'
import type { ColumnDef } from '@/components/common/DataTable'
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
  if (!src) return 'No source'
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
      onClick={(e) => {
        e.stopPropagation()
        onChange(!checked)
      }}
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

  // Column definitions
  const columns: ColumnDef<Workflow>[] = [
    {
      key: 'name',
      header: 'Workflow',
      render: (wf) => (
        <span className="text-sm font-medium text-gray-900 dark:text-white">{wf.name}</span>
      ),
    },
    {
      key: 'agent',
      header: 'Agent',
      render: (wf) => {
        const agent = agentMap.get(wf.agent_id)
        return agent ? (
          <div className="flex items-center gap-2">
            <AgentStatusBadge status={agent.status} variant="dot" />
            <span className="text-xs text-gray-700 dark:text-gray-300">{agent.name}</span>
          </div>
        ) : (
          <span className="text-xs text-gray-400 dark:text-gray-500">Unknown agent</span>
        )
      },
    },
    {
      key: 'source',
      header: 'Source',
      render: (wf) => (
        <span className="text-xs text-gray-500 dark:text-gray-400">{sourceLabel(wf)}</span>
      ),
    },
    {
      key: 'interval',
      header: 'Interval',
      render: (wf) => (
        <span className="text-xs text-gray-500 dark:text-gray-400">
          {formatInterval(wf.poll_interval_secs)}
        </span>
      ),
    },
    {
      key: 'enabled',
      header: 'Enabled',
      render: (wf) => (
        <ToggleSwitch
          checked={wf.enabled}
          onChange={(v) => handleToggle(wf.id, v)}
          disabled={togglingId === wf.id}
          label={`${wf.enabled ? 'Disable' : 'Enable'} ${wf.name}`}
        />
      ),
    },
    {
      key: 'updated_at',
      header: 'Updated',
      render: (wf) => (
        <span className="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
          {new Date(wf.updated_at).toLocaleDateString()}
        </span>
      ),
    },
    {
      key: 'actions',
      header: '',
      render: (wf) => (
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
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
      ),
    },
  ]

  return (
    <>
      <DataTable
        columns={columns}
        data={workflows}
        rowKey={(wf) => wf.id}
        loading={loading}
        onRowClick={(wf) => navigate(`/workflows/${wf.id}`)}
        emptyTitle="No workflows configured yet."
        emptyDescription="Create a workflow to start dispatching tasks automatically."
      />

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
