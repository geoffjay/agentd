/**
 * CreateAgentDialog — slide-over / modal to create a new agent.
 *
 * Covers all fields from the CreateAgentRequest spec:
 *   name, working_dir, model, shell, interactive, prompt,
 *   system_prompt, tool_policy, env (key-value pairs)
 */

import { useState } from 'react'
import { Plus, Trash2, X } from 'lucide-react'
import type { CreateAgentRequest, ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MODELS = [
  { label: 'Default', value: '' },
  { label: 'claude-sonnet-4-5-20251001', value: 'claude-sonnet-4-5-20251001' },
  { label: 'claude-opus-4-5-20251001', value: 'claude-opus-4-5-20251001' },
  { label: 'claude-haiku-4-5-20251001', value: 'claude-haiku-4-5-20251001' },
]

const TOOL_POLICY_TYPES = [
  { label: 'Allow All', value: 'AllowAll' },
  { label: 'Deny All', value: 'DenyAll' },
  { label: 'Allow List', value: 'AllowList' },
  { label: 'Deny List', value: 'DenyList' },
  { label: 'Require Approval', value: 'RequireApproval' },
]

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FormState {
  name: string
  working_dir: string
  model: string
  shell: string
  interactive: boolean
  prompt: string
  system_prompt: string
  tool_policy_type: string
  tool_list: string // comma-separated tool names
  env_keys: string[]
  env_values: string[]
}

interface FormErrors {
  name?: string
  working_dir?: string
  general?: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildToolPolicy(type: string, toolList: string): ToolPolicy {
  if (type === 'AllowList') {
    return {
      type: 'AllowList',
      tools: toolList
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
    }
  }
  if (type === 'DenyList') {
    return {
      type: 'DenyList',
      tools: toolList
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
    }
  }
  return { type: type as 'AllowAll' | 'DenyAll' | 'RequireApproval' }
}

function buildRequest(form: FormState): CreateAgentRequest {
  const env: Record<string, string> = {}
  form.env_keys.forEach((key, i) => {
    if (key.trim()) env[key.trim()] = form.env_values[i] ?? ''
  })

  return {
    name: form.name.trim(),
    working_dir: form.working_dir.trim(),
    model: form.model || undefined,
    shell: form.shell.trim() || '/bin/sh',
    interactive: form.interactive,
    prompt: form.interactive ? undefined : form.prompt.trim() || undefined,
    system_prompt: form.system_prompt.trim() || undefined,
    tool_policy: buildToolPolicy(form.tool_policy_type, form.tool_list),
    env: Object.keys(env).length > 0 ? env : undefined,
  }
}

function validate(form: FormState): FormErrors {
  const errors: FormErrors = {}
  if (!form.name.trim()) errors.name = 'Name is required.'
  if (!form.working_dir.trim()) errors.working_dir = 'Working directory is required.'
  return errors
}

// ---------------------------------------------------------------------------
// Section label
// ---------------------------------------------------------------------------

function SectionLabel({
  htmlFor,
  label,
  optional,
}: {
  htmlFor: string
  label: string
  optional?: boolean
}) {
  return (
    <label htmlFor={htmlFor} className="block text-sm font-medium text-gray-700 dark:text-gray-300">
      {label}
      {optional && (
        <span className="ml-1 text-xs font-normal text-gray-400 dark:text-gray-500">
          (optional)
        </span>
      )}
    </label>
  )
}

function FieldError({ msg }: { msg?: string }) {
  if (!msg) return null
  return <p className="mt-1 text-xs text-red-600 dark:text-red-400">{msg}</p>
}

// ---------------------------------------------------------------------------
// CreateAgentDialog
// ---------------------------------------------------------------------------

export interface CreateAgentDialogProps {
  open: boolean
  onClose: () => void
  onCreate: (request: CreateAgentRequest) => Promise<void>
}

const DEFAULT_FORM: FormState = {
  name: '',
  working_dir: '',
  model: '',
  shell: '',
  interactive: false,
  prompt: '',
  system_prompt: '',
  tool_policy_type: 'AllowAll',
  tool_list: '',
  env_keys: [''],
  env_values: [''],
}

export function CreateAgentDialog({ open, onClose, onCreate }: CreateAgentDialogProps) {
  const [form, setForm] = useState<FormState>(DEFAULT_FORM)
  const [errors, setErrors] = useState<FormErrors>({})
  const [submitting, setSubmitting] = useState(false)

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }))
    if (key in errors) {
      setErrors((prev) => ({ ...prev, [key]: undefined }))
    }
  }

  function addEnvRow() {
    setForm((prev) => ({
      ...prev,
      env_keys: [...prev.env_keys, ''],
      env_values: [...prev.env_values, ''],
    }))
  }

  function removeEnvRow(i: number) {
    setForm((prev) => ({
      ...prev,
      env_keys: prev.env_keys.filter((_, idx) => idx !== i),
      env_values: prev.env_values.filter((_, idx) => idx !== i),
    }))
  }

  function updateEnvKey(i: number, value: string) {
    const keys = [...form.env_keys]
    keys[i] = value
    setForm((prev) => ({ ...prev, env_keys: keys }))
  }

  function updateEnvValue(i: number, value: string) {
    const values = [...form.env_values]
    values[i] = value
    setForm((prev) => ({ ...prev, env_values: values }))
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    const errs = validate(form)
    if (Object.keys(errs).length > 0) {
      setErrors(errs)
      return
    }

    setSubmitting(true)
    setErrors({})
    try {
      await onCreate(buildRequest(form))
      setForm(DEFAULT_FORM)
      onClose()
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to create agent'
      setErrors({ general: msg })
    } finally {
      setSubmitting(false)
    }
  }

  function handleClose() {
    if (submitting) return
    setForm(DEFAULT_FORM)
    setErrors({})
    onClose()
  }

  if (!open) return null

  const showToolList = form.tool_policy_type === 'AllowList' || form.tool_policy_type === 'DenyList'

  const inputCls =
    'block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 placeholder:text-gray-400 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white dark:placeholder:text-gray-500'

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-end">
      {/* Backdrop */}
      <div aria-hidden="true" className="absolute inset-0 bg-black/40" onClick={handleClose} />

      {/* Panel */}
      <aside
        aria-label="Create Agent"
        role="dialog"
        aria-modal="true"
        aria-labelledby="create-agent-title"
        className="relative flex h-full w-full max-w-lg flex-col bg-white shadow-2xl dark:bg-gray-900"
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-gray-200 px-6 py-4 dark:border-gray-700">
          <h2
            id="create-agent-title"
            className="text-lg font-semibold text-gray-900 dark:text-white"
          >
            Create Agent
          </h2>
          <button
            type="button"
            aria-label="Close"
            onClick={handleClose}
            className="rounded-md p-1.5 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800 dark:hover:text-gray-300"
          >
            <X size={18} />
          </button>
        </div>

        {/* Form */}
        <form
          id="create-agent-form"
          onSubmit={handleSubmit}
          className="flex-1 overflow-y-auto"
          noValidate
        >
          <div className="space-y-5 px-6 py-5">
            {/* General error */}
            {errors.general && (
              <div className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
                {errors.general}
              </div>
            )}

            {/* Name */}
            <div>
              <SectionLabel htmlFor="agent-name" label="Name" />
              <input
                id="agent-name"
                type="text"
                required
                value={form.name}
                onChange={(e) => update('name', e.target.value)}
                placeholder="my-agent"
                className={[inputCls, errors.name ? 'border-red-500 focus:ring-red-500' : ''].join(
                  ' ',
                )}
              />
              <FieldError msg={errors.name} />
            </div>

            {/* Working Directory */}
            <div>
              <SectionLabel htmlFor="agent-working-dir" label="Working Directory" />
              <input
                id="agent-working-dir"
                type="text"
                required
                value={form.working_dir}
                onChange={(e) => update('working_dir', e.target.value)}
                placeholder="/home/user/project"
                className={[
                  inputCls,
                  errors.working_dir ? 'border-red-500 focus:ring-red-500' : '',
                ].join(' ')}
              />
              <FieldError msg={errors.working_dir} />
            </div>

            {/* Model */}
            <div>
              <SectionLabel htmlFor="agent-model" label="Model" optional />
              <select
                id="agent-model"
                value={form.model}
                onChange={(e) => update('model', e.target.value)}
                className={inputCls}
              >
                {MODELS.map((m) => (
                  <option key={m.value} value={m.value}>
                    {m.label}
                  </option>
                ))}
              </select>
            </div>

            {/* Interactive toggle */}
            <div className="flex items-center justify-between">
              <div>
                <span className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                  Interactive
                </span>
                <span className="text-xs text-gray-400 dark:text-gray-500">
                  Run agent in interactive (TTY) mode
                </span>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={form.interactive}
                aria-label="Interactive mode"
                onClick={() => update('interactive', !form.interactive)}
                className={[
                  'relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900',
                  form.interactive ? 'bg-primary-600' : 'bg-gray-200 dark:bg-gray-700',
                ].join(' ')}
              >
                <span
                  className={[
                    'inline-block h-4 w-4 rounded-full bg-white shadow transition-transform',
                    form.interactive ? 'translate-x-6' : 'translate-x-1',
                  ].join(' ')}
                />
              </button>
            </div>

            {/* Prompt — only shown when non-interactive */}
            {!form.interactive && (
              <div>
                <SectionLabel htmlFor="agent-prompt" label="Prompt" optional />
                <textarea
                  id="agent-prompt"
                  rows={3}
                  value={form.prompt}
                  onChange={(e) => update('prompt', e.target.value)}
                  placeholder="Initial prompt for the agent…"
                  className={[inputCls, 'resize-none'].join(' ')}
                />
              </div>
            )}

            {/* System Prompt */}
            <div>
              <SectionLabel htmlFor="agent-system-prompt" label="System Prompt" optional />
              <textarea
                id="agent-system-prompt"
                rows={3}
                value={form.system_prompt}
                onChange={(e) => update('system_prompt', e.target.value)}
                placeholder="System prompt override…"
                className={[inputCls, 'resize-none'].join(' ')}
              />
            </div>

            {/* Tool Policy */}
            <div className="space-y-2">
              <SectionLabel htmlFor="agent-tool-policy" label="Tool Policy" />
              <select
                id="agent-tool-policy"
                value={form.tool_policy_type}
                onChange={(e) => update('tool_policy_type', e.target.value)}
                className={inputCls}
              >
                {TOOL_POLICY_TYPES.map((t) => (
                  <option key={t.value} value={t.value}>
                    {t.label}
                  </option>
                ))}
              </select>
              {showToolList && (
                <div>
                  <label
                    htmlFor="agent-tool-list"
                    className="block text-xs text-gray-500 dark:text-gray-400"
                  >
                    Tool names (comma-separated)
                  </label>
                  <input
                    id="agent-tool-list"
                    type="text"
                    value={form.tool_list}
                    onChange={(e) => update('tool_list', e.target.value)}
                    placeholder="bash, read_file, write_file"
                    className={inputCls}
                  />
                </div>
              )}
            </div>

            {/* Shell */}
            <div>
              <SectionLabel htmlFor="agent-shell" label="Shell" optional />
              <input
                id="agent-shell"
                type="text"
                value={form.shell}
                onChange={(e) => update('shell', e.target.value)}
                placeholder="/bin/bash"
                className={inputCls}
              />
            </div>

            {/* Environment Variables */}
            <div>
              <span className="block text-sm font-medium text-gray-700 dark:text-gray-300">
                Environment Variables{' '}
                <span className="text-xs font-normal text-gray-400 dark:text-gray-500">
                  (optional)
                </span>
              </span>
              <div className="mt-2 space-y-2">
                {form.env_keys.map((key, i) => (
                  <div key={i} className="flex items-center gap-2">
                    <input
                      type="text"
                      aria-label={`Environment variable key ${i + 1}`}
                      value={key}
                      onChange={(e) => updateEnvKey(i, e.target.value)}
                      placeholder="KEY"
                      className={[inputCls, 'flex-1 font-mono text-xs'].join(' ')}
                    />
                    <span className="text-gray-400">=</span>
                    <input
                      type="text"
                      aria-label={`Environment variable value ${i + 1}`}
                      value={form.env_values[i] ?? ''}
                      onChange={(e) => updateEnvValue(i, e.target.value)}
                      placeholder="value"
                      className={[inputCls, 'flex-1 font-mono text-xs'].join(' ')}
                    />
                    <button
                      type="button"
                      aria-label={`Remove environment variable ${i + 1}`}
                      onClick={() => removeEnvRow(i)}
                      disabled={form.env_keys.length === 1}
                      className="rounded p-1 text-gray-400 hover:text-red-500 disabled:opacity-30"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                ))}
                <button
                  type="button"
                  onClick={addEnvRow}
                  className="flex items-center gap-1 text-xs text-primary-600 hover:text-primary-700 dark:text-primary-400 dark:hover:text-primary-300"
                >
                  <Plus size={12} />
                  Add variable
                </button>
              </div>
            </div>
          </div>
        </form>

        {/* Footer */}
        <div className="flex justify-end gap-3 border-t border-gray-200 px-6 py-4 dark:border-gray-700">
          <button
            type="button"
            onClick={handleClose}
            disabled={submitting}
            className="rounded-md border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 disabled:opacity-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
          >
            Cancel
          </button>
          <button
            type="submit"
            form="create-agent-form"
            disabled={submitting}
            className="rounded-md bg-primary-600 px-4 py-2 text-sm font-medium text-white hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2 disabled:opacity-50 transition-colors"
          >
            {submitting ? 'Creating…' : 'Create Agent'}
          </button>
        </div>
      </aside>
    </div>
  )
}

export default CreateAgentDialog
