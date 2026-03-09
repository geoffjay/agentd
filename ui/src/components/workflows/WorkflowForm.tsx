/**
 * WorkflowForm — create/edit workflow dialog.
 *
 * Fields:
 * - Name
 * - Agent (dropdown filtered to Running agents)
 * - Task source type (GitHub Issues)
 * - GitHub Issues config: owner, repo, labels, state
 * - Prompt template (with PromptTemplateEditor)
 * - Poll interval (in minutes)
 * - Enabled toggle
 *
 * Validation: agent must be selected, GitHub fields required, interval ≥ 1m.
 */

import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import { PromptTemplateEditor } from './PromptTemplateEditor'
import type { Agent } from '@/types/orchestrator'
import type { CreateWorkflowRequest, Workflow } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface WorkflowFormProps {
  open: boolean
  /** Provided when editing; undefined when creating */
  workflow?: Workflow
  /** List of available agents */
  agents: Agent[]
  onSave: (request: CreateWorkflowRequest) => Promise<void>
  onClose: () => void
}

interface FormErrors {
  name?: string
  agent_id?: string
  owner?: string
  repo?: string
  prompt_template?: string
  poll_interval?: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_TEMPLATE =
  `You are working on the following task:\n\nTitle: {{title}}\n\nDescription:\n{{body}}\n\nSource: {{url}}\nLabels: {{labels}}\n\nPlease work on this task and report back when complete.`

function secsToMinutes(secs: number): string {
  const mins = Math.round(secs / 60)
  return String(mins)
}

function minutesToSecs(mins: string): number {
  const n = parseInt(mins, 10)
  return isNaN(n) ? 60 : Math.max(1, n) * 60
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function WorkflowForm({ open, workflow, agents, onSave, onClose }: WorkflowFormProps) {
  const firstFieldRef = useRef<HTMLInputElement>(null)

  // Form state
  const [name, setName] = useState('')
  const [agentId, setAgentId] = useState('')
  const [owner, setOwner] = useState('')
  const [repo, setRepo] = useState('')
  const [labelsRaw, setLabelsRaw] = useState('')
  const [issueState, setIssueState] = useState<'open' | 'closed' | 'all'>('open')
  const [promptTemplate, setPromptTemplate] = useState(DEFAULT_TEMPLATE)
  const [pollMinutes, setPollMinutes] = useState('15')
  const [enabled, setEnabled] = useState(true)
  const [errors, setErrors] = useState<FormErrors>({})
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | undefined>()

  // Populate form when editing
  useEffect(() => {
    if (!open) return
    if (workflow) {
      setName(workflow.name)
      setAgentId(workflow.agent_id)
      const src = workflow.source_config
      if (src.type === 'github_issues') {
        setOwner(src.owner)
        setRepo(src.repo)
        setLabelsRaw(src.labels.join(', '))
        setIssueState(src.state as 'open' | 'closed' | 'all')
      }
      setPromptTemplate(workflow.prompt_template)
      setPollMinutes(secsToMinutes(workflow.poll_interval_secs))
      setEnabled(workflow.enabled)
    } else {
      setName('')
      setAgentId('')
      setOwner('')
      setRepo('')
      setLabelsRaw('')
      setIssueState('open')
      setPromptTemplate(DEFAULT_TEMPLATE)
      setPollMinutes('15')
      setEnabled(true)
    }
    setErrors({})
    setSaveError(undefined)
    // Focus first field after render
    setTimeout(() => firstFieldRef.current?.focus(), 50)
  }, [open, workflow])

  // Close on Escape
  useEffect(() => {
    if (!open) return
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [open, onClose])

  if (!open) return null

  // Validation
  function validate(): FormErrors {
    const e: FormErrors = {}
    if (!name.trim()) e.name = 'Name is required'
    if (!agentId) e.agent_id = 'Select an agent'
    if (!owner.trim()) e.owner = 'Owner is required'
    if (!repo.trim()) e.repo = 'Repository is required'
    if (!promptTemplate.trim()) e.prompt_template = 'Prompt template is required'
    const mins = parseInt(pollMinutes, 10)
    if (isNaN(mins) || mins < 1) e.poll_interval = 'Minimum poll interval is 1 minute'
    return e
  }

  async function handleSave() {
    const e = validate()
    if (Object.keys(e).length > 0) {
      setErrors(e)
      return
    }

    setSaving(true)
    setSaveError(undefined)
    try {
      const labels = labelsRaw
        .split(',')
        .map((l) => l.trim())
        .filter(Boolean)

      const request: CreateWorkflowRequest = {
        name: name.trim(),
        agent_id: agentId,
        source_config: {
          type: 'github_issues',
          owner: owner.trim(),
          repo: repo.trim(),
          labels,
          state: issueState,
        },
        prompt_template: promptTemplate.trim(),
        poll_interval_secs: minutesToSecs(pollMinutes),
        enabled,
        tool_policy: { mode: 'allow_all' },
      }

      await onSave(request)
      onClose()
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save workflow')
    } finally {
      setSaving(false)
    }
  }

  const runningAgents = agents.filter((a) => a.status === 'running')
  const isEditing = Boolean(workflow)

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="workflow-form-title"
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50"
        onClick={onClose}
        aria-hidden="true"
      />

      {/* Panel */}
      <div className="relative z-10 w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-xl bg-white dark:bg-gray-900 shadow-xl">
        {/* Header */}
        <div className="sticky top-0 z-10 flex items-center justify-between border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 px-6 py-4">
          <h2 id="workflow-form-title" className="text-lg font-semibold text-gray-900 dark:text-white">
            {isEditing ? 'Edit Workflow' : 'Create Workflow'}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded p-1 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500"
            aria-label="Close dialog"
          >
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="px-6 py-5 space-y-5">
          {saveError && (
            <p className="rounded-md bg-red-50 dark:bg-red-900/20 px-3 py-2 text-sm text-red-700 dark:text-red-400">
              {saveError}
            </p>
          )}

          {/* Name */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Workflow name <span className="text-red-500">*</span>
            </label>
            <input
              ref={firstFieldRef}
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Dispatch GitHub Issues"
              className={fieldClass(errors.name)}
            />
            {errors.name && <p className="mt-1 text-xs text-red-500">{errors.name}</p>}
          </div>

          {/* Agent */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Agent <span className="text-red-500">*</span>
            </label>
            <select
              value={agentId}
              onChange={(e) => setAgentId(e.target.value)}
              className={fieldClass(errors.agent_id)}
            >
              <option value="">Select a running agent…</option>
              {runningAgents.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
            {runningAgents.length === 0 && (
              <p className="mt-1 text-xs text-yellow-600 dark:text-yellow-400">
                No running agents found. Start an agent first.
              </p>
            )}
            {errors.agent_id && <p className="mt-1 text-xs text-red-500">{errors.agent_id}</p>}
          </div>

          {/* Task source type */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Task source type
            </label>
            <select
              value="github_issues"
              disabled
              className={fieldClass()}
            >
              <option value="github_issues">GitHub Issues</option>
            </select>
            <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
              More source types planned for future releases.
            </p>
          </div>

          {/* GitHub Issues config */}
          <fieldset className="rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-3">
            <legend className="text-sm font-medium text-gray-700 dark:text-gray-300 px-1">
              GitHub Issues configuration
            </legend>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  Owner <span className="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  value={owner}
                  onChange={(e) => setOwner(e.target.value)}
                  placeholder="e.g. geoffjay"
                  className={fieldClass(errors.owner, 'text-sm')}
                />
                {errors.owner && <p className="mt-1 text-xs text-red-500">{errors.owner}</p>}
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  Repository <span className="text-red-500">*</span>
                </label>
                <input
                  type="text"
                  value={repo}
                  onChange={(e) => setRepo(e.target.value)}
                  placeholder="e.g. agentd"
                  className={fieldClass(errors.repo, 'text-sm')}
                />
                {errors.repo && <p className="mt-1 text-xs text-red-500">{errors.repo}</p>}
              </div>
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                Labels <span className="text-gray-400">(comma-separated)</span>
              </label>
              <input
                type="text"
                value={labelsRaw}
                onChange={(e) => setLabelsRaw(e.target.value)}
                placeholder="e.g. bug, enhancement"
                className={fieldClass(undefined, 'text-sm')}
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                Issue state
              </label>
              <select
                value={issueState}
                onChange={(e) => setIssueState(e.target.value as 'open' | 'closed' | 'all')}
                className={fieldClass(undefined, 'text-sm')}
              >
                <option value="open">Open</option>
                <option value="closed">Closed</option>
                <option value="all">All</option>
              </select>
            </div>
          </fieldset>

          {/* Prompt template */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Prompt template <span className="text-red-500">*</span>
            </label>
            <PromptTemplateEditor
              value={promptTemplate}
              onChange={setPromptTemplate}
              disabled={saving}
              error={errors.prompt_template}
            />
          </div>

          {/* Poll interval + Enabled */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Poll interval (minutes) <span className="text-red-500">*</span>
              </label>
              <input
                type="number"
                min={1}
                value={pollMinutes}
                onChange={(e) => setPollMinutes(e.target.value)}
                className={fieldClass(errors.poll_interval)}
              />
              {errors.poll_interval && (
                <p className="mt-1 text-xs text-red-500">{errors.poll_interval}</p>
              )}
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                Enabled
              </label>
              <button
                type="button"
                role="switch"
                aria-checked={enabled}
                onClick={() => setEnabled((v) => !v)}
                className={[
                  'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-500',
                  enabled
                    ? 'bg-primary-500'
                    : 'bg-gray-200 dark:bg-gray-700',
                ].join(' ')}
              >
                <span
                  className={[
                    'inline-block h-4 w-4 rounded-full bg-white shadow transition-transform',
                    enabled ? 'translate-x-6' : 'translate-x-1',
                  ].join(' ')}
                />
              </button>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="sticky bottom-0 flex items-center justify-end gap-3 border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 px-6 py-4">
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            className="rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 transition-colors disabled:opacity-50"
          >
            {saving ? 'Saving…' : isEditing ? 'Save changes' : 'Create workflow'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

function fieldClass(error?: string, extra = ''): string {
  return [
    'w-full rounded-md border px-3 py-2 text-sm',
    'bg-white dark:bg-gray-900',
    'text-gray-900 dark:text-white',
    'focus:outline-none focus:ring-2 focus:ring-primary-500',
    'disabled:cursor-not-allowed disabled:opacity-50',
    error
      ? 'border-red-400 dark:border-red-500'
      : 'border-gray-300 dark:border-gray-600',
    extra,
  ]
    .filter(Boolean)
    .join(' ')
}

export default WorkflowForm
