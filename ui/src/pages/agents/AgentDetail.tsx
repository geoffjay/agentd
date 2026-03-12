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

import { useEffect, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import {
  ArrowLeft,
  ChevronDown,
  Copy,
  Eraser,
  FolderPlus,
  Loader2,
  MoreHorizontal,
  RefreshCw,
  Settings2,
  Trash2,
} from 'lucide-react'
import { AgentStatusBadge } from '@/components/agents/AgentStatusBadge'
import { AgentConfigPanel } from '@/components/agents/AgentConfigPanel'
import { AgentLogView } from '@/components/agents/AgentLogView'
import { AgentCommandInput } from '@/components/agents/AgentCommandInput'
import { AgentPolicyEditor } from '@/components/agents/AgentPolicyEditor'
import { AgentApprovals } from '@/components/agents/AgentApprovals'
import { AgentUsagePanel } from '@/components/agents/AgentUsagePanel'
import { ConfirmDialog } from '@/components/common/ConfirmDialog'
import { CardSkeleton } from '@/components/common/LoadingSkeleton'
import { useAgentDetail } from '@/hooks/useAgentDetail'
import { useAgentStream } from '@/hooks/useAgentStream'
import { useAgentUsage } from '@/hooks/useAgentUsage'
import { useToast } from '@/hooks/useToast'
import { orchestratorClient } from '@/services/orchestrator'
import type { SessionUsage, SetModelRequest, ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Model selector constants
// ---------------------------------------------------------------------------

const MODELS = [
  { label: 'Default (server)', value: '' },
  { label: 'Claude Sonnet 4.6', value: 'claude-sonnet-4-6' },
  { label: 'Claude Opus 4.6', value: 'claude-opus-4-6' },
  { label: 'Claude Haiku 4.6', value: 'claude-haiku-4-6' },
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
      <div className="absolute inset-0 bg-black/50" aria-hidden="true" onClick={onClose} />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="change-model-title"
        className="relative rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2
          id="change-model-title"
          className="mb-4 text-base font-semibold text-gray-900 dark:text-white"
        >
          Change Model
        </h2>

        {error && (
          <p role="alert" className="mb-3 text-sm text-red-400 dark:text-red-400">
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
              onChange={(e) => setModel(e.target.value)}
              className={inputCls}
            >
              {MODELS.map((m) => (
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
              onChange={(e) => setRestart(e.target.checked)}
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
// Clear Context dialog — shows session summary before clearing
// ---------------------------------------------------------------------------

const costFmt = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 4, maximumFractionDigits: 4 })

interface ClearContextDialogProps {
  open: boolean
  session?: SessionUsage
  loading: boolean
  onConfirm: () => void
  onCancel: () => void
}

function ClearContextDialog({ open, session, loading, onConfirm, onCancel }: ClearContextDialogProps) {
  if (!open) return null

  const totalTokens = session
    ? session.input_tokens + session.output_tokens + session.cache_read_input_tokens + session.cache_creation_input_tokens
    : 0

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/50" aria-hidden="true" onClick={onCancel} />
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="clear-context-title"
        aria-describedby="clear-context-desc"
        className="relative rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2
          id="clear-context-title"
          className="text-base font-semibold text-gray-900 dark:text-white"
        >
          Clear context?
        </h2>

        <p id="clear-context-desc" className="mt-2 text-sm text-gray-500 dark:text-gray-400">
          This will clear the agent&apos;s current context and start a new session. Current session usage will be saved.
        </p>

        {/* Session stats summary */}
        {session && totalTokens > 0 && (
          <div className="mt-3 rounded-md bg-gray-50 p-3 dark:bg-gray-700/50">
            <p className="mb-1.5 text-xs font-medium text-gray-500 dark:text-gray-400">
              Current session
            </p>
            <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              <span className="text-gray-500 dark:text-gray-400">Input tokens</span>
              <span className="text-right font-medium text-gray-700 dark:text-gray-300">
                {session.input_tokens.toLocaleString()}
              </span>
              <span className="text-gray-500 dark:text-gray-400">Output tokens</span>
              <span className="text-right font-medium text-gray-700 dark:text-gray-300">
                {session.output_tokens.toLocaleString()}
              </span>
              <span className="text-gray-500 dark:text-gray-400">Cache tokens</span>
              <span className="text-right font-medium text-gray-700 dark:text-gray-300">
                {(session.cache_read_input_tokens + session.cache_creation_input_tokens).toLocaleString()}
              </span>
              <span className="text-gray-500 dark:text-gray-400">Cost</span>
              <span className="text-right font-medium text-gray-700 dark:text-gray-300">
                {costFmt.format(session.total_cost_usd)}
              </span>
              <span className="text-gray-500 dark:text-gray-400">Turns</span>
              <span className="text-right font-medium text-gray-700 dark:text-gray-300">
                {session.num_turns}
              </span>
            </div>
          </div>
        )}

        <div className="mt-5 flex justify-end gap-3">
          <button
            type="button"
            onClick={onCancel}
            disabled={loading}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={loading}
            className="rounded-md bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-700 focus:outline-none focus:ring-2 focus:ring-amber-500 focus:ring-offset-2 disabled:opacity-50 transition-colors"
          >
            {loading ? 'Clearing…' : 'Clear Context'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Add Directory dialog
// ---------------------------------------------------------------------------

interface AddDirDialogProps {
  open: boolean
  onConfirm: (path: string) => Promise<void>
  onClose: () => void
}

function AddDirDialog({ open, onConfirm, onClose }: AddDirDialogProps) {
  const [path, setPath] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | undefined>()

  if (!open) return null

  async function handleSubmit() {
    const trimmed = path.trim()
    if (!trimmed) return
    setSaving(true)
    setError(undefined)
    try {
      await onConfirm(trimmed)
      setPath('')
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add directory')
    } finally {
      setSaving(false)
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter') handleSubmit()
    if (e.key === 'Escape') onClose()
  }

  const inputCls =
    'block w-full rounded-md border border-gray-300 bg-white px-3 py-2 font-mono text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/50" aria-hidden="true" onClick={onClose} />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-dir-header-title"
        className="relative w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2
          id="add-dir-header-title"
          className="mb-4 text-base font-semibold text-gray-900 dark:text-white"
        >
          Add Directory
        </h2>

        <p className="mb-3 text-sm text-gray-500 dark:text-gray-400">
          Enter an absolute path to grant the agent access via{' '}
          <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs dark:bg-gray-700">
            --add-dir
          </code>
          .
        </p>

        {error && (
          <p role="alert" className="mb-3 text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        )}

        <div className="flex flex-col gap-2">
          <label
            htmlFor="add-dir-header-path"
            className="text-sm font-medium text-gray-700 dark:text-gray-300"
          >
            Directory path
          </label>
          <input
            id="add-dir-header-path"
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="/path/to/directory"
            className={inputCls}
            // eslint-disable-next-line jsx-a11y/no-autofocus
            autoFocus
            disabled={saving}
          />
        </div>

        <p className="mt-3 text-xs text-amber-600 dark:text-amber-400">
          Directory changes take effect on the next agent restart.
        </p>

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
            onClick={handleSubmit}
            disabled={saving || !path.trim()}
            className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 disabled:opacity-50 transition-colors"
          >
            {saving ? 'Adding…' : 'Add Directory'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Actions dropdown menu
// ---------------------------------------------------------------------------

interface ActionsDropdownProps {
  isRunning: boolean
  clearing: boolean
  onChangeModel: () => void
  onAddDir: () => void
  onClearContext: () => void
  onTerminate: () => void
}

function ActionsDropdown({
  isRunning,
  clearing,
  onChangeModel,
  onAddDir,
  onClearContext,
  onTerminate,
}: ActionsDropdownProps) {
  const [open, setOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)

  // Close on outside click
  useEffect(() => {
    if (!open) return
    function handlePointerDown(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handlePointerDown)
    return () => document.removeEventListener('mousedown', handlePointerDown)
  }, [open])

  // Escape to close, arrow keys to navigate
  useEffect(() => {
    if (!open) return
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        setOpen(false)
        buttonRef.current?.focus()
        return
      }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault()
        const items = Array.from(
          menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])') ?? [],
        )
        if (items.length === 0) return
        const idx = items.indexOf(document.activeElement as HTMLElement)
        if (e.key === 'ArrowDown') {
          items[(idx + 1) % items.length].focus()
        } else {
          items[(idx - 1 + items.length) % items.length].focus()
        }
      }
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [open])

  function pick(fn: () => void) {
    setOpen(false)
    fn()
  }

  const itemCls =
    'flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm focus:outline-none'
  const normalItem = `${itemCls} text-gray-700 hover:bg-gray-50 focus:bg-gray-50 dark:text-gray-300 dark:hover:bg-gray-700/50 dark:focus:bg-gray-700/50`
  const amberItem = `${itemCls} text-amber-700 hover:bg-amber-50 focus:bg-amber-50 dark:text-amber-400 dark:hover:bg-amber-900/20 dark:focus:bg-amber-900/20 disabled:opacity-50 disabled:cursor-not-allowed`
  const redItem = `${itemCls} text-red-600 hover:bg-red-50 focus:bg-red-50 dark:text-red-400 dark:hover:bg-red-900/20 dark:focus:bg-red-900/20`

  return (
    <div ref={menuRef} className="relative">
      <button
        ref={buttonRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1.5 rounded-md border border-gray-300 bg-white px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
      >
        <MoreHorizontal size={14} aria-hidden="true" />
        <span className="hidden sm:inline">Actions</span>
        <ChevronDown
          size={12}
          aria-hidden="true"
          className={`transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <div
          role="menu"
          aria-label="Agent actions"
          className="absolute right-0 z-20 mt-1 w-52 rounded-md border border-gray-200 bg-white py-1 shadow-lg dark:border-gray-700 dark:bg-gray-800"
        >
          {/* Change Model */}
          <button
            role="menuitem"
            type="button"
            onClick={() => pick(onChangeModel)}
            className={normalItem}
          >
            <Settings2 size={14} className="text-gray-400 dark:text-gray-500" aria-hidden="true" />
            Change Model
          </button>

          {/* Add Directory */}
          <button
            role="menuitem"
            type="button"
            onClick={() => pick(onAddDir)}
            className={normalItem}
          >
            <FolderPlus size={14} className="text-gray-400 dark:text-gray-500" aria-hidden="true" />
            Add Directory
          </button>

          {/* Clear Context (amber / warning) */}
          <button
            role="menuitem"
            type="button"
            onClick={() => pick(onClearContext)}
            disabled={!isRunning || clearing}
            className={amberItem}
          >
            {clearing ? (
              <Loader2
                size={14}
                className="animate-spin text-amber-500"
                aria-hidden="true"
              />
            ) : (
              <Eraser size={14} className="text-amber-500" aria-hidden="true" />
            )}
            Clear Context
          </button>

          {/* Divider */}
          <div role="separator" className="my-1 border-t border-gray-100 dark:border-gray-700" />

          {/* Terminate (red / danger) */}
          <button
            role="menuitem"
            type="button"
            onClick={() => pick(onTerminate)}
            className={redItem}
          >
            <Trash2 size={14} className="text-red-500" aria-hidden="true" />
            Terminate
          </button>
        </div>
      )}
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
  const { usage, clearContext, clearing } = useAgentUsage(agentId)
  const toast = useToast()

  const [confirmTerminate, setConfirmTerminate] = useState(false)
  const [terminating, setTerminating] = useState(false)
  const [confirmClearContext, setConfirmClearContext] = useState(false)
  const [showModelDialog, setShowModelDialog] = useState(false)
  const [showAddDirDialog, setShowAddDirDialog] = useState(false)
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

  async function handleAddDir(path: string) {
    await orchestratorClient.addDir(agentId, path)
    await refetch()
  }

  async function handleRemoveDir(path: string) {
    await orchestratorClient.removeDir(agentId, path)
    await refetch()
  }

  async function handleClearContext() {
    try {
      const response = await clearContext()
      setConfirmClearContext(false)
      toast.success('Context cleared', {
        message: `New session #${response.new_session_number} started`,
      })
    } catch (err) {
      toast.error('Failed to clear context', {
        message: err instanceof Error ? err.message : 'An unknown error occurred',
      })
    }
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

  const isRunning = agent.status === 'running'
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
              <h1 className="text-xl font-semibold text-gray-900 dark:text-white">{agent.name}</h1>
              <AgentStatusBadge status={agent.status} />
            </div>

            {/* ID */}
            <div className="flex items-center gap-1.5">
              <span className="font-mono text-xs text-gray-400 dark:text-gray-500">{agentId}</span>
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
              {agent.config.model && <span>Model: {agent.config.model}</span>}
            </div>
          </div>

          {/* Right: actions — Refresh (standalone) + Actions dropdown */}
          <div className="flex flex-shrink-0 items-center gap-2">
            {/* Refresh — kept standalone: frequent, low-risk, no confirmation */}
            <button
              type="button"
              aria-label="Refresh agent data"
              onClick={refetch}
              className="rounded-md border border-gray-300 bg-white p-2 text-gray-500 hover:bg-gray-50 hover:text-gray-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700"
            >
              <RefreshCw size={14} aria-hidden="true" />
            </button>

            {/* Actions dropdown */}
            <ActionsDropdown
              isRunning={isRunning}
              clearing={clearing}
              onChangeModel={() => setShowModelDialog(true)}
              onAddDir={() => setShowAddDirDialog(true)}
              onClearContext={() => setConfirmClearContext(true)}
              onTerminate={() => setConfirmTerminate(true)}
            />
          </div>
        </div>
      </div>

      {/* ── Main content + sidebar ───────────────────────────────────────── */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-3">
        {/* Log view (takes 2/3 width on large screens) */}
        <div className="flex flex-col gap-3 lg:col-span-2">
          <div className="h-[480px]">
            <AgentLogView lines={lines} status={streamStatus} onClear={clearLog} />
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

          {/* Config panel */}
          <AgentConfigPanel
            agent={agent}
            onAddDir={handleAddDir}
            onRemoveDir={handleRemoveDir}
          />
        </div>

        {/* Sidebar (1/3 width on large screens) */}
        <div className="flex flex-col gap-5">
          {/* Usage panel — shown only when usage data is available */}
          {usage && (
            <AgentUsagePanel
              usage={usage}
              autoClearThreshold={agent.config.auto_clear_threshold}
            />
          )}

          {/* Tool policy */}
          <section
            aria-label="Tool policy"
            className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900"
          >
            <div className="flex items-center justify-between border-b border-gray-100 px-4 py-3 dark:border-gray-700">
              <h2 className="text-sm font-medium text-gray-900 dark:text-white">Tool Policy</h2>
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
                <AgentPolicyEditor policy={agent.config.tool_policy} onSave={handlePolicySave} />
              ) : (
                <p className="text-sm text-gray-700 dark:text-gray-300">
                  {(() => {
                    const p = agent.config.tool_policy
                    switch (p.mode) {
                      case 'allow_all':
                        return 'Allow all tools'
                      case 'deny_all':
                        return 'Deny all tools'
                      case 'require_approval':
                        return 'Require approval for all tools'
                      case 'allow_list':
                        return `Allow: ${p.tools.join(', ') || '(none)'}`
                      case 'deny_list':
                        return `Deny: ${p.tools.join(', ') || '(none)'}`
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

      <ClearContextDialog
        open={confirmClearContext}
        session={usage?.current_session}
        loading={clearing}
        onConfirm={handleClearContext}
        onCancel={() => setConfirmClearContext(false)}
      />

      <ChangeModelDialog
        open={showModelDialog}
        currentModel={agent.config.model}
        onSave={handleModelSave}
        onClose={() => setShowModelDialog(false)}
      />

      <AddDirDialog
        open={showAddDirDialog}
        onConfirm={handleAddDir}
        onClose={() => setShowAddDirDialog(false)}
      />
    </div>
  )
}

export default AgentDetail
