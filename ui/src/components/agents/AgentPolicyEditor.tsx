/**
 * AgentPolicyEditor — inline editor for an agent's tool policy.
 *
 * Allows:
 * - Selecting policy type from dropdown
 * - For AllowList/DenyList: editing a comma-separated list of tool names
 * - Saving via onSave callback (parent handles API call)
 */

import { useState } from 'react'
import { Save } from 'lucide-react'
import type { ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AgentPolicyEditorProps {
  policy: ToolPolicy
  saving?: boolean
  onSave: (policy: ToolPolicy) => Promise<void>
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const POLICY_TYPES = [
  { label: 'Allow All', value: 'AllowAll' },
  { label: 'Deny All', value: 'DenyAll' },
  { label: 'Allow List', value: 'AllowList' },
  { label: 'Deny List', value: 'DenyList' },
  { label: 'Require Approval', value: 'RequireApproval' },
] as const

type PolicyType = (typeof POLICY_TYPES)[number]['value']

function getToolsString(policy: ToolPolicy): string {
  if (policy.type === 'AllowList' || policy.type === 'DenyList') {
    return policy.tools.join(', ')
  }
  return ''
}

function buildPolicy(type: PolicyType, toolsStr: string): ToolPolicy {
  if (type === 'AllowList') {
    return {
      type: 'AllowList',
      tools: toolsStr
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
    }
  }
  if (type === 'DenyList') {
    return {
      type: 'DenyList',
      tools: toolsStr
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
    }
  }
  return { type }
}

const inputCls =
  'block w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-1 focus:ring-primary-500 dark:border-gray-600 dark:bg-gray-800 dark:text-white'

// ---------------------------------------------------------------------------
// AgentPolicyEditor
// ---------------------------------------------------------------------------

export function AgentPolicyEditor({ policy, saving = false, onSave }: AgentPolicyEditorProps) {
  const [type, setType] = useState<PolicyType>(policy.type)
  const [toolsStr, setToolsStr] = useState(getToolsString(policy))
  const [error, setError] = useState<string | undefined>()

  const showToolList = type === 'AllowList' || type === 'DenyList'

  async function handleSave() {
    setError(undefined)
    try {
      await onSave(buildPolicy(type, toolsStr))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save policy')
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      )}

      {/* Policy type dropdown */}
      <div>
        <label
          htmlFor="policy-type"
          className="mb-1 block text-xs font-medium text-gray-500 dark:text-gray-400"
        >
          Policy type
        </label>
        <select
          id="policy-type"
          value={type}
          onChange={(e) => setType(e.target.value as PolicyType)}
          disabled={saving}
          className={inputCls}
        >
          {POLICY_TYPES.map((p) => (
            <option key={p.value} value={p.value}>
              {p.label}
            </option>
          ))}
        </select>
      </div>

      {/* Tool list (only for AllowList / DenyList) */}
      {showToolList && (
        <div>
          <label
            htmlFor="policy-tools"
            className="mb-1 block text-xs font-medium text-gray-500 dark:text-gray-400"
          >
            Tool names (comma-separated)
          </label>
          <input
            id="policy-tools"
            type="text"
            value={toolsStr}
            onChange={(e) => setToolsStr(e.target.value)}
            disabled={saving}
            placeholder="bash, read_file, write_file"
            className={inputCls}
          />
        </div>
      )}

      {/* Save button */}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="flex items-center gap-1.5 rounded-md bg-primary-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-primary-700 focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1 disabled:opacity-50 transition-colors"
        >
          <Save size={14} aria-hidden="true" />
          {saving ? 'Saving…' : 'Save Policy'}
        </button>
      </div>
    </div>
  )
}

export default AgentPolicyEditor
