/**
 * AgentConfigPanel — collapsible panel showing agent configuration details.
 *
 * Displays:
 * - Working directory, shell, interactive mode
 * - System prompt (truncated, expandable)
 * - Tool policy (human-readable summary)
 * - Environment variables (values masked by default)
 * - Worktree info (if present)
 * - Model, tmux session
 * - Additional directories (with add/remove controls)
 */

import { useState } from 'react'
import { ChevronDown, ChevronRight, Eye, EyeOff, FolderOpen, Plus, X } from 'lucide-react'
import type { Agent, ToolPolicy } from '@/types/orchestrator'
import { HighlightedCode } from '@/components/common'

// ---------------------------------------------------------------------------
// Tool policy display helper
// ---------------------------------------------------------------------------

function policyLabel(policy: ToolPolicy): string {
  switch (policy.mode) {
    case 'allow_all':
      return 'Allow All tools'
    case 'deny_all':
      return 'Deny All tools'
    case 'require_approval':
      return 'Require Approval for all tools'
    case 'allow_list':
      return `Allow: ${policy.tools.length > 0 ? policy.tools.join(', ') : '(none)'}`
    case 'deny_list':
      return `Deny: ${policy.tools.length > 0 ? policy.tools.join(', ') : '(none)'}`
  }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ConfigRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
      <span className="w-36 flex-shrink-0 text-xs font-medium text-gray-400 dark:text-gray-500">
        {label}
      </span>
      <span className="text-sm text-gray-700 dark:text-gray-300">{children}</span>
    </div>
  )
}

function EnvVarsRow({ env }: { env: Record<string, string> }) {
  const [revealed, setRevealed] = useState(false)
  const entries = Object.entries(env)
  if (entries.length === 0) return null

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-gray-400 dark:text-gray-500">Environment</span>
        <button
          type="button"
          aria-label={revealed ? 'Hide env values' : 'Show env values'}
          onClick={() => setRevealed((v) => !v)}
          className="rounded p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
        >
          {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
        </button>
      </div>
      <div className="ml-0 flex flex-col gap-1 pl-0 font-mono text-xs">
        {entries.map(([key, value]) => (
          <div key={key} className="flex gap-2">
            <span className="text-gray-500 dark:text-gray-400">{key}=</span>
            <span className="text-gray-700 dark:text-gray-300">
              {revealed ? value : '••••••••'}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function SystemPromptRow({ prompt }: { prompt: string }) {
  const [expanded, setExpanded] = useState(false)
  const isLong = prompt.length > 200

  return (
    <div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
      <span className="w-36 flex-shrink-0 text-xs font-medium text-gray-400 dark:text-gray-500">
        System Prompt
      </span>
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        {expanded ? (
          <HighlightedCode
            code={prompt}
            language="markdown"
            maxHeight="20rem"
            className="border border-gray-200 dark:border-gray-700"
          />
        ) : (
          <p className="whitespace-pre-wrap text-sm text-gray-700 dark:text-gray-300">
            {isLong ? `${prompt.slice(0, 200)}…` : prompt}
          </p>
        )}
        {isLong && (
          <button
            type="button"
            onClick={() => setExpanded((e) => !e)}
            className="self-start text-xs text-primary-600 hover:text-primary-700 dark:text-primary-400"
          >
            {expanded ? 'Show less' : 'Show more'}
          </button>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Add Directory dialog (inline)
// ---------------------------------------------------------------------------

interface AddDirDialogProps {
  open: boolean
  saving: boolean
  error?: string
  onConfirm: (path: string) => void
  onCancel: () => void
}

function AddDirDialog({ open, saving, error, onConfirm, onCancel }: AddDirDialogProps) {
  const [path, setPath] = useState('')

  if (!open) return null

  function handleSubmit() {
    const trimmed = path.trim()
    if (trimmed) {
      onConfirm(trimmed)
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter') handleSubmit()
    if (e.key === 'Escape') onCancel()
  }

  const inputCls =
    'block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 font-mono focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/50" aria-hidden="true" onClick={onCancel} />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-dir-title"
        className="relative rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2
          id="add-dir-title"
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
            htmlFor="add-dir-path"
            className="text-sm font-medium text-gray-700 dark:text-gray-300"
          >
            Directory path
          </label>
          <input
            id="add-dir-path"
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="/path/to/directory"
            className={inputCls}
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
            onClick={onCancel}
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
// Additional Directories row
// ---------------------------------------------------------------------------

interface AdditionalDirsRowProps {
  dirs: string[]
  onAdd?: (path: string) => Promise<void>
  onRemove?: (path: string) => Promise<void>
}

function AdditionalDirsRow({ dirs, onAdd, onRemove }: AdditionalDirsRowProps) {
  const [showDialog, setShowDialog] = useState(false)
  const [saving, setSaving] = useState(false)
  const [addError, setAddError] = useState<string | undefined>()
  const [removingPath, setRemovingPath] = useState<string | undefined>()
  const [restartNotice, setRestartNotice] = useState(false)

  async function handleAdd(path: string) {
    if (!onAdd) return
    setSaving(true)
    setAddError(undefined)
    try {
      await onAdd(path)
      setShowDialog(false)
      setRestartNotice(true)
      setTimeout(() => setRestartNotice(false), 5000)
    } catch (err) {
      setAddError(err instanceof Error ? err.message : 'Failed to add directory')
    } finally {
      setSaving(false)
    }
  }

  async function handleRemove(path: string) {
    if (!onRemove) return
    setRemovingPath(path)
    try {
      await onRemove(path)
      setRestartNotice(true)
      setTimeout(() => setRestartNotice(false), 5000)
    } catch {
      // Silently ignore — the list will not update on error
    } finally {
      setRemovingPath(undefined)
    }
  }

  const canEdit = Boolean(onAdd && onRemove)

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <span className="w-36 flex-shrink-0 text-xs font-medium text-gray-400 dark:text-gray-500">
          Additional Dirs
        </span>
        {canEdit && (
          <button
            type="button"
            aria-label="Add directory"
            onClick={() => {
              setAddError(undefined)
              setShowDialog(true)
            }}
            className="flex items-center gap-1 rounded p-0.5 text-xs text-primary-600 hover:text-primary-700 dark:text-primary-400"
          >
            <Plus size={13} />
            Add
          </button>
        )}
      </div>

      {dirs.length === 0 ? (
        <span className="pl-0 text-sm text-gray-400 dark:text-gray-500 sm:pl-40">(none)</span>
      ) : (
        <ul className="flex flex-col gap-1 pl-0 sm:pl-40">
          {dirs.map((dir) => (
            <li key={dir} className="flex items-center gap-2 group">
              <FolderOpen
                size={13}
                className="flex-shrink-0 text-gray-400 dark:text-gray-500"
                aria-hidden="true"
              />
              <span className="flex-1 font-mono text-xs text-gray-700 dark:text-gray-300 break-all">
                {dir}
              </span>
              {canEdit && (
                <button
                  type="button"
                  aria-label={`Remove directory ${dir}`}
                  onClick={() => handleRemove(dir)}
                  disabled={removingPath === dir}
                  className="rounded p-0.5 text-gray-300 hover:text-red-500 disabled:opacity-50 dark:text-gray-600 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                >
                  <X size={13} />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}

      {restartNotice && (
        <p className="pl-0 text-xs text-amber-600 dark:text-amber-400 sm:pl-40">
          Directory changes take effect on the next agent restart.
        </p>
      )}

      <AddDirDialog
        open={showDialog}
        saving={saving}
        error={addError}
        onConfirm={handleAdd}
        onCancel={() => setShowDialog(false)}
      />
    </div>
  )
}

// ---------------------------------------------------------------------------
// AgentConfigPanel
// ---------------------------------------------------------------------------

export interface AgentConfigPanelProps {
  agent: Agent
  onAddDir?: (path: string) => Promise<void>
  onRemoveDir?: (path: string) => Promise<void>
}

export function AgentConfigPanel({ agent, onAddDir, onRemoveDir }: AgentConfigPanelProps) {
  const [open, setOpen] = useState(true)
  const { config } = agent

  return (
    <section
      aria-label="Agent configuration"
      className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900"
    >
      {/* Header / toggle */}
      <button
        type="button"
        aria-expanded={open}
        aria-controls="agent-config-body"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center justify-between px-4 py-3 text-sm font-medium text-gray-900 hover:bg-gray-50 dark:text-white dark:hover:bg-gray-800/50"
      >
        <span>Configuration</span>
        {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
      </button>

      {open && (
        <div
          id="agent-config-body"
          className="flex flex-col gap-3 border-t border-gray-100 px-4 py-4 dark:border-gray-700"
        >
          <ConfigRow label="Working Dir">
            <span className="font-mono text-xs">{config.working_dir}</span>
          </ConfigRow>

          <ConfigRow label="Shell">
            <span className="font-mono text-xs">{config.shell}</span>
          </ConfigRow>

          <ConfigRow label="Interactive">
            {config.interactive ? (
              <span className="text-green-600 dark:text-green-400">Yes (TTY)</span>
            ) : (
              <span className="text-gray-500 dark:text-gray-400">No</span>
            )}
          </ConfigRow>

          {config.model && (
            <ConfigRow label="Model">
              <span className="font-mono text-xs">{config.model}</span>
            </ConfigRow>
          )}

          {config.worktree && (
            <ConfigRow label="Worktree">
              <span className="font-mono text-xs">{config.worktree}</span>
            </ConfigRow>
          )}

          {agent.session_id && (
            <ConfigRow label="Session">
              <span className="font-mono text-xs">{agent.session_id}</span>
            </ConfigRow>
          )}

          <ConfigRow label="Tool Policy">{policyLabel(config.tool_policy)}</ConfigRow>

          <ConfigRow label="Auto-clear">
            {config.auto_clear_threshold != null && config.auto_clear_threshold > 0 ? (
              <span className="text-amber-600 dark:text-amber-400">
                at {config.auto_clear_threshold.toLocaleString()} tokens
              </span>
            ) : (
              <span className="text-gray-500 dark:text-gray-400">Disabled</span>
            )}
          </ConfigRow>

          {config.system_prompt && <SystemPromptRow prompt={config.system_prompt} />}

          {config.env && Object.keys(config.env).length > 0 && <EnvVarsRow env={config.env} />}

          <AdditionalDirsRow
            dirs={config.additional_dirs ?? []}
            onAdd={onAddDir}
            onRemove={onRemoveDir}
          />
        </div>
      )}
    </section>
  )
}

export default AgentConfigPanel
