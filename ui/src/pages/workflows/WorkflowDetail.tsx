/**
 * WorkflowDetail — detail view for a single workflow.
 *
 * Layout:
 * - Header: name, enabled status, agent, created/updated timestamps, actions
 * - Configuration card: source config, prompt template, poll interval
 * - Dispatch history table
 */

import { useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Edit2, RefreshCw, Trash2, Zap } from 'lucide-react'
import { DispatchHistory } from '@/components/workflows/DispatchHistory'
import { WorkflowForm } from '@/components/workflows/WorkflowForm'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import { StatusBadge } from '@/components/common/StatusBadge'
import { useWorkflowDetail } from '@/hooks/useWorkflows'
import { useAgents } from '@/hooks/useAgents'
import type { CreateWorkflowRequest } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sourceDetail(
  src: { type: string; owner?: string; repo?: string; labels?: string[]; state?: string } | undefined,
): string {
  if (!src) return 'No source configured'
  if (src.type === 'github_issues') {
    const parts: string[] = []
    if (src.owner && src.repo) parts.push(`${src.owner}/${src.repo}`)
    if (src.labels && src.labels.length > 0) parts.push(`Labels: ${src.labels.join(', ')}`)
    if (src.state) parts.push(`State: ${src.state}`)
    return parts.join(' · ')
  }
  return src.type
}

function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// ---------------------------------------------------------------------------
// Config detail card
// ---------------------------------------------------------------------------

function ConfigRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="grid grid-cols-3 gap-4 py-3 border-t border-gray-100 dark:border-gray-800 first:border-t-0">
      <dt className="text-sm font-medium text-gray-500 dark:text-gray-400">{label}</dt>
      <dd className="col-span-2 text-sm text-gray-900 dark:text-white">{value}</dd>
    </div>
  )
}

// ---------------------------------------------------------------------------
// WorkflowDetail
// ---------------------------------------------------------------------------

export function WorkflowDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  const { workflow, loading, error, refetch, updateWorkflow, deleteWorkflow } =
    useWorkflowDetail(id ?? '')

  const { allAgents } = useAgents({ pageSize: 200 })
  const [formOpen, setFormOpen] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [deleting, setDeleting] = useState(false)

  async function handleSave(request: CreateWorkflowRequest) {
    await updateWorkflow({
      name: request.name,
      prompt_template: request.prompt_template,
      poll_interval_secs: request.poll_interval_secs,
      enabled: request.enabled,
      tool_policy: request.tool_policy,
    })
  }

  async function handleDelete() {
    setDeleting(true)
    try {
      await deleteWorkflow()
      navigate('/workflows')
    } finally {
      setDeleting(false)
      setConfirmDelete(false)
    }
  }

  if (loading) {
    return (
      <div id="main-content" className="space-y-4">
        <CardSkeleton />
        <CardSkeleton />
      </div>
    )
  }

  if (error || !workflow) {
    return (
      <div id="main-content" className="space-y-4">
        <Link
          to="/workflows"
          className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white transition-colors"
        >
          <ArrowLeft size={16} />
          Back to Workflows
        </Link>
        <p className="text-sm text-red-500 dark:text-red-400">
          {error ?? 'Workflow not found.'}
        </p>
      </div>
    )
  }

  const agent = allAgents.find((a) => a.id === workflow.agent_id)

  return (
    <div id="main-content" className="space-y-6">
      {/* Back nav */}
      <Link
        to="/workflows"
        className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500 rounded"
      >
        <ArrowLeft size={16} />
        Back to Workflows
      </Link>

      {/* Header */}
      <div className="flex items-start justify-between">
        <div className="flex items-start gap-4">
          <div className="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-xl bg-primary-100 dark:bg-primary-900/30">
            <Zap size={24} className="text-primary-600 dark:text-primary-400" />
          </div>
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">
                {workflow.name}
              </h1>
              <StatusBadge status={workflow.enabled ? 'healthy' : 'unknown'} />
            </div>
            <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
              Agent: {agent?.name ?? workflow.agent_id}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={refetch}
            className="rounded-md p-2 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            aria-label="Refresh"
          >
            <RefreshCw size={18} />
          </button>
          <button
            type="button"
            onClick={() => setFormOpen(true)}
            className="inline-flex items-center gap-2 rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
          >
            <Edit2 size={15} />
            Edit
          </button>
          <button
            type="button"
            onClick={() => setConfirmDelete(true)}
            className="inline-flex items-center gap-2 rounded-md border border-red-300 dark:border-red-700 bg-white dark:bg-gray-800 px-3 py-2 text-sm font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-500"
          >
            <Trash2 size={15} />
            Delete
          </button>
        </div>
      </div>

      {/* Configuration card */}
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-6">
        <h2 className="text-base font-semibold text-gray-900 dark:text-white mb-4">
          Configuration
        </h2>
        <dl>
          <ConfigRow label="Source" value={sourceDetail(workflow.source_config)} />
          <ConfigRow
            label="Poll interval"
            value={
              workflow.poll_interval_secs < 60
                ? `${workflow.poll_interval_secs}s`
                : `${Math.round(workflow.poll_interval_secs / 60)}m`
            }
          />
          <ConfigRow label="Enabled" value={workflow.enabled ? 'Yes' : 'No'} />
          <ConfigRow label="Created" value={formatDateTime(workflow.created_at)} />
          <ConfigRow label="Updated" value={formatDateTime(workflow.updated_at)} />
          <ConfigRow
            label="Prompt template"
            value={
              <pre className="text-xs font-mono bg-gray-50 dark:bg-gray-900 rounded p-2 whitespace-pre-wrap max-h-32 overflow-auto">
                {workflow.prompt_template}
              </pre>
            }
          />
        </dl>
      </div>

      {/* Dispatch history */}
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-6">
        <h2 className="text-base font-semibold text-gray-900 dark:text-white mb-4">
          Dispatch history
        </h2>
        <DispatchHistory workflowId={workflow.id} />
      </div>

      {/* Edit dialog */}
      <WorkflowForm
        open={formOpen}
        workflow={workflow}
        agents={allAgents}
        onSave={handleSave}
        onClose={() => setFormOpen(false)}
      />

      {/* Delete confirmation */}
      <ConfirmDialog
        open={confirmDelete}
        title="Delete workflow"
        description={`Delete "${workflow.name}"? This cannot be undone.`}
        confirmLabel="Delete"
        variant="danger"
        loading={deleting}
        onConfirm={handleDelete}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  )
}

export default WorkflowDetail
