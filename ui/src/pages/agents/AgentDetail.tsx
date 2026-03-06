/**
 * AgentDetail — detail view for a single agent.
 *
 * Layout:
 * ┌─ Header: name, status, ID, timestamps, model, actions ────────────────┐
 * │  ┌─ Main (log + command) ──────────────┐  ┌─ Sidebar ───────────────┐ │
 * │  │  AgentLogView                       │  │  AgentConfigPanel       │ │
 * │  │  AgentCommandInput                  │  │  Tool Policy            │ │
 * │  └─────────────────────────────────────┘  │  Pending Approvals      │ │
 * │                                           └─────────────────────────┘ │
 * └───────────────────────────────────────────────────────────────────────┘
 */

import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Copy, RefreshCw, Settings2, Trash2 } from 'lucide-react'
import { AgentStatusBadge } from '@/components/agents/AgentStatusBadge'
import { AgentConfigPanel } from '@/components/agents/AgentConfigPanel'
import { AgentLogView } from '@/components/agents/AgentLogView'
import { AgentCommandInput } from '@/components/agents/AgentCommandInput'
import { AgentPolicyEditor } from '@/components/agents/AgentPolicyEditor'
import { AgentApprovals } from '@/components/agents/AgentApprovals'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import { useAgentDetail } from '@/hooks/useAgentDetail'
import { useAgentStream } from '@/hooks/useAgentStream'
import type { SetModelRequest, ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Model selector constants
// ---------------------------------------------------------------------------

const MODELS = [
  { label: 'Default (server)', value: '' },
  { label: 'claude-sonnet-4-5-20251001', value: 'claude-sonnet-4-5-20251001' },
  { label: 'claude-opus-4-5-20251001', value: 'claude-opus-4-5-20251001' },
  { label: 'claude-haiku-4-5-20251001', value: 'claude-haiku-4-5-20251001' },
]

// ---------------------------------------------------------------------------
// Change Model dialog
// ---------------------------------------------------------------------------

interface ChangeModelDialogProps {
  open: boolean
  currentModel?: string
  onSave: (request: SetModelRequest) => Promise<void>
  onClose: () => void
}

function ChangeModelDialog({ open, currentModel, onSave, onClose }: ChangeModelDialogProps) {
  const [model, setModel] = useState(currentModel ?? '')
  const [restart, setRestart] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | undefined>()

  if (!open) return null

  async function handleSave() {
    setSaving(true)
    setError(undefined)
    try {
      await onSave({ model: model || undefined, restart })
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update model')
    } finally {
      setSaving(false)
    }
  }

  const inputCls =
    'block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-black/50"
        aria-hidden="true"
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="change-model-title"
        className="relative w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2
          id="change-model-title"
          className="mb-4 text-base font-semibold text-gray-900 dark:text-white"
        >
          Change Model
        </h2>

        {error && (
          <p role="alert" className="mb-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <div className="flex flex-col gap-3">
          <div>
            <label
              htmlFor="change-model-select"
              className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300"
            >
              Model
            </label>
            <select
              id="change-model-select"
              value={model}
              onChange={e => setModel(e.target.value)}
              className={inputCls}
            >
              {MODELS.map(m => (
                <option key={m.value} value={m.value}>
                  {m.label}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-2">
            <input
              id="change-model-restart"
              type="checkbox"
              checked={restart}
              onChange={e => setRestart(e.target.checked)}
              className="h-4 w-4 rounded border-gray-300 text-primary-600"
            />
            <label
              htmlFor="change-model-restart"
              className="text-sm text-gray-700 dark:text-gray-300"
            >
              Restart agent after change
            </label>
          </div>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={saving}
            className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 disabled:opacity-50 transition-colors"
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// AgentDetail
// ---------------------------------------------------------------------------

export function AgentDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()

  const agentId = id ?? ''

  const {
    agent,
    loading,
    error,
    refetch,
    deleteAgent,
    sendMessage,
    updateModel,
    updatePolicy,
    approvals,
    approvalsLoading,
    approvalsError,
    approveRequest,
    denyRequest,
  } = useAgentDetail(agentId)

  const { lines, status: streamStatus, clear: clearLog } = useAgentStream(agentId)

  const [confirmTerminate, setConfirmTerminate] = useState(false)
  const [terminating, setTerminating] = useState(false)
  const [showModelDialog, setShowModelDialog] = useState(false)
  const [policyEditing, setPolicyEditing] = useState(false)
  const [copied, setCopied] = useState(false)

  // ---------------------------------------------------------------------------
  // Handlers
  // ---------------------------------------------------------------------------

  async function handleTerminate() {
    setTerminating(true)
    try {
      await deleteAgent()
      navigate('/agents')
    } catch {
      // Navigate anyway — the agent may already be gone
      navigate('/agents')
    } finally {
      setTerminating(false)
      setConfirmTerminate(false)
    }
  }

  async function handleModelSave(request: SetModelRequest) {
    await updateModel(request)
    setShowModelDialog(false)
  }

  async function handlePolicySave(policy: ToolPolicy) {
    await updatePolicy(policy)
    setPolicyEditing(false)
  }

  function copyId() {
    if (!agentId) return
    navigator.clipboard.writeText(agentId).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }

  // ---------------------------------------------------------------------------
  // Loading / error state
  // ---------------------------------------------------------------------------

  if (loading) {
    return (
      <div className="space-y-4">
        <CardSkeleton />
        <CardSkeleton />
      </div>
    )
  }

  if (error || !agent) {
    return (
      <div className="space-y-4">
        <button
          type="button"
          onClick={() => navigate('/agents')}
          className="flex items-center gap-1.5 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          <ArrowLeft size={14} />
          Back to agents
        </button>
        <div
          role="alert"
          className="rounded-md bg-red-50 px-4 py-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400"
        >
          {error ?? 'Agent not found'}
        </div>
      </div>
    )
  }

  const isRunning = agent.status === 'Running'
  const canSendMessage = isRunning && !agent.config.interactive

  const formattedCreated = new Date(agent.created_at).toLocaleString()
  const formattedUpdated = new Date(agent.updated_at).toLocaleString()

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div className="flex flex-col gap-5">
      {/* Back link */}
      <button
        type="button"
        onClick={() => navigate('/agents')}
        className="flex items-center gap-1.5 self-start text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
      >
        <ArrowLeft size={14} aria-hidden="true" />
        Back to agents
      </button>

      {/* ── Agent header ────────────────────────────────────────────────── */}
      <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-900">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          {/* Left: identity */}
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-3">
              <h1 className="text-xl font-semibold text-gray-900 dark:text-white">
                {agent.name}
              </h1>
              <AgentStatusBadge status={agent.status} />
            </div>

            {/* ID */}
            <div className="flex items-center gap-1.5">
              <span className="font-mono text-xs text-gray-400 dark:text-gray-500">
                {agentId}
              </span>
              <button
                type="button"
                aria-label="Copy agent ID"
                onClick={copyId}
                className="rounded p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                <Copy size={12} />
              </button>
              {copied && (
                <span className="text-xs text-green-500 dark:text-green-400">Copied!</span>
              )}
            </div>

            {/* Timestamps & model */}
            <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-gray-500 dark:text-gray-400">
              <span>Created: {formattedCreated}</span>
              <span>Updated: {formattedUpdated}</span>
              {agent.config.model && (
                <span>Model: {agent.config.model}</span>
              )}
            </div>
          </div>

          {/* Right: actions */}
          <div className="flex flex-shrink-0 items-center gap-2">
            {/* Refresh */}
            <button
              type="button"
              aria-label="Refresh agent data"
              onClick={refetch}
              className="rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700"
            >
              <RefreshCw size={14} aria-hidden="true" />
            </button>

            {/* Change model */}
            <button
              type="button"
              onClick={() => setShowModelDialog(true)}
              className="flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
            >
              <Settings2 size={14} aria-hidden="true" />
              Model
            </button>

            {/* Terminate */}
            <button
              type="button"
              onClick={() => setConfirmTerminate(true)}
              className="flex items-center gap-1.5 rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-1 transition-colors"
            >
              <Trash2 size={14} aria-hidden="true" />
              Terminate
            </button>
          </div>
        </div>
      </div>

      {/* ── Main content + sidebar ───────────────────────────────────────── */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-3">
        {/* Log view (takes 2/3 width on large screens) */}
        <div className="flex flex-col gap-3 lg:col-span-2">
          <div className="h-[480px]">
            <AgentLogView
              lines={lines}
              status={streamStatus}
              onClear={clearLog}
            />
          </div>

          {/* Command input */}
          <AgentCommandInput
            agentId={agentId}
            enabled={canSendMessage}
            disabledReason={
              !isRunning
                ? 'Agent is not running'
                : agent.config.interactive
                  ? 'Interactive agents do not accept commands here'
                  : undefined
            }
            onSend={sendMessage}
          />
        </div>

        {/* Sidebar (1/3 width on large screens) */}
        <div className="flex flex-col gap-5">
          {/* Config panel */}
          <AgentConfigPanel agent={agent} />

          {/* Tool policy */}
          <section
            aria-label="Tool policy"
            className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900"
          >
            <div className="flex items-center justify-between border-b border-gray-100 px-4 py-3 dark:border-gray-700">
              <h2 className="text-sm font-medium text-gray-900 dark:text-white">
                Tool Policy
              </h2>
              {!policyEditing && (
                <button
                  type="button"
                  onClick={() => setPolicyEditing(true)}
                  className="text-xs text-primary-600 hover:text-primary-700 dark:text-primary-400"
                >
                  Edit
                </button>
              )}
            </div>
            <div className="p-4">
              {policyEditing ? (
                <AgentPolicyEditor
                  policy={agent.config.tool_policy}
                  onSave={handlePolicySave}
                />
              ) : (
                <p className="text-sm text-gray-700 dark:text-gray-300">
                  {(() => {
                    const p = agent.config.tool_policy
                    switch (p.type) {
                      case 'AllowAll': return 'Allow all tools'
                      case 'DenyAll': return 'Deny all tools'
                      case 'RequireApproval': return 'Require approval for all tools'
                      case 'AllowList': return `Allow: ${p.tools.join(', ') || '(none)'}`
                      case 'DenyList': return `Deny: ${p.tools.join(', ') || '(none)'}`
                    }
                  })()}
                </p>
              )}
            </div>
          </section>

          {/* Pending approvals */}
          <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
            <AgentApprovals
              approvals={approvals}
              loading={approvalsLoading}
              error={approvalsError}
              onApprove={approveRequest}
              onDeny={denyRequest}
            />
          </div>
        </div>
      </div>

      {/* Dialogs */}
      <ConfirmDialog
        open={confirmTerminate}
        title="Terminate agent?"
        description={`This will permanently terminate "${agent.name}" and all associated resources. This action cannot be undone.`}
        confirmLabel="Terminate"
        variant="danger"
        loading={terminating}
        onConfirm={handleTerminate}
        onCancel={() => setConfirmTerminate(false)}
      />

      <ChangeModelDialog
        open={showModelDialog}
        currentModel={agent.config.model}
        onSave={handleModelSave}
        onClose={() => setShowModelDialog(false)}
      />
    </div>
  )
}

export default AgentDetail
