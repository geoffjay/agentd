/**
 * WorkflowList — workflow management page.
 *
 * Shows a table of all configured workflows with create, edit, delete,
 * and enable/disable actions. Includes search and pagination.
 */

import { useState } from 'react'
import { Plus, RefreshCw, Search } from 'lucide-react'
import { WorkflowTable } from '@/components/workflows/WorkflowTable'
import { WorkflowForm } from '@/components/workflows/WorkflowForm'
import { useWorkflows } from '@/hooks/useWorkflows'
import { useAgents } from '@/hooks/useAgents'
import type { CreateWorkflowRequest, Workflow } from '@/types/orchestrator'

export function WorkflowList() {
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(1)
  const [formOpen, setFormOpen] = useState(false)
  const [editingWorkflow, setEditingWorkflow] = useState<Workflow | undefined>()

  const {
    workflows,
    total,
    loading,
    refreshing,
    error,
    refetch,
    createWorkflow,
    updateWorkflow,
    deleteWorkflow,
    toggleEnabled,
  } = useWorkflows({ search, page, pageSize: 20, paused: formOpen })

  // We need the full agent list for the form dropdown
  const { allAgents } = useAgents({ pageSize: 200 })

  function openCreate() {
    setEditingWorkflow(undefined)
    setFormOpen(true)
  }

  function openEdit(workflow: Workflow) {
    setEditingWorkflow(workflow)
    setFormOpen(true)
  }

  async function handleSave(request: CreateWorkflowRequest) {
    if (editingWorkflow) {
      await updateWorkflow(editingWorkflow.id, {
        name: request.name,
        prompt_template: request.prompt_template,
        poll_interval_secs: request.poll_interval_secs,
        enabled: request.enabled,
        tool_policy: request.tool_policy,
      })
    } else {
      await createWorkflow(request)
    }
    setPage(1)
  }

  const pageCount = Math.ceil(total / 20)

  return (
    <div id="main-content" className="space-y-6">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Workflows</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
            Automated task dispatch — poll external sources and dispatch tasks to agents.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={refetch}
            disabled={loading || refreshing}
            className="rounded-md p-2 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors disabled:opacity-50"
            aria-label="Refresh workflows"
          >
            <RefreshCw size={18} className={refreshing ? 'animate-spin' : ''} />
          </button>
          <button
            type="button"
            onClick={openCreate}
            className="inline-flex items-center gap-2 rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
          >
            <Plus size={16} />
            New workflow
          </button>
        </div>
      </div>

      {/* Error banner */}
      {error && (
        <div className="rounded-md bg-red-50 dark:bg-red-900/20 px-4 py-3 text-sm text-red-700 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Search */}
      <div className="relative max-w-xs">
        <Search
          size={14}
          className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none"
        />
        <input
          type="text"
          value={search}
          onChange={(e) => { setSearch(e.target.value); setPage(1) }}
          placeholder="Search workflows…"
          className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 pl-8 pr-3 py-2 text-sm text-gray-900 dark:text-white placeholder:text-gray-400 focus:outline-none focus:ring-2 focus:ring-primary-500"
        />
      </div>

      {/* Table */}
      <WorkflowTable
        workflows={workflows}
        agents={allAgents}
        loading={loading}
        onEdit={openEdit}
        onDelete={deleteWorkflow}
        onToggleEnabled={toggleEnabled}
      />

      {/* Pagination */}
      {pageCount > 1 && (
        <div className="flex items-center justify-between text-sm text-gray-500 dark:text-gray-400">
          <span>{total} workflow{total !== 1 ? 's' : ''}</span>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={page <= 1}
              onClick={() => setPage((p) => p - 1)}
              className="rounded px-3 py-1 border border-gray-300 dark:border-gray-600 disabled:opacity-40 hover:bg-gray-50 dark:hover:bg-gray-800"
            >
              Previous
            </button>
            <span className="px-2 py-1">
              Page {page} of {pageCount}
            </span>
            <button
              type="button"
              disabled={page >= pageCount}
              onClick={() => setPage((p) => p + 1)}
              className="rounded px-3 py-1 border border-gray-300 dark:border-gray-600 disabled:opacity-40 hover:bg-gray-50 dark:hover:bg-gray-800"
            >
              Next
            </button>
          </div>
        </div>
      )}

      {/* Create/Edit form dialog */}
      <WorkflowForm
        open={formOpen}
        workflow={editingWorkflow}
        agents={allAgents}
        onSave={handleSave}
        onClose={() => setFormOpen(false)}
      />
    </div>
  )
}

export default WorkflowList
