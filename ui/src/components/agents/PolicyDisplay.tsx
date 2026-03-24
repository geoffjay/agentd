/**
 * PolicyDisplay — read-only view of an agent's tool policy.
 *
 * Shown in the Tool Policy section when the user is not actively editing.
 * Pairs with AgentPolicyEditor which is shown during editing.
 */

import type { ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface PolicyDisplayProps {
  policy: ToolPolicy
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MODE_LABELS: Record<ToolPolicy['mode'], string> = {
  allow_all: 'Allow All',
  deny_all: 'Deny All',
  allow_list: 'Allow List',
  deny_list: 'Deny List',
  require_approval: 'Require Approval',
}

// ---------------------------------------------------------------------------
// PolicyDisplay
// ---------------------------------------------------------------------------

export function PolicyDisplay({ policy }: PolicyDisplayProps) {
  const label = MODE_LABELS[policy.mode]
  const tools = (policy.mode === 'allow_list' || policy.mode === 'deny_list') ? policy.tools : []

  return (
    <dl className="flex flex-col gap-2 text-sm">
      <div className="flex items-center gap-2">
        <dt className="text-xs font-medium text-gray-500 dark:text-gray-400 w-24 shrink-0">
          Policy type
        </dt>
        <dd className="font-medium text-gray-900 dark:text-white">{label}</dd>
      </div>

      {tools.length > 0 && (
        <div className="flex items-start gap-2">
          <dt className="text-xs font-medium text-gray-500 dark:text-gray-400 w-24 shrink-0 pt-0.5">
            Tools
          </dt>
          <dd className="flex flex-wrap gap-1">
            {tools.map((tool) => (
              <span
                key={tool}
                className="inline-flex items-center rounded bg-gray-100 px-2 py-0.5 text-xs font-mono text-gray-700 dark:bg-gray-700 dark:text-gray-300"
              >
                {tool}
              </span>
            ))}
          </dd>
        </div>
      )}

      {tools.length === 0 && policy.mode !== 'allow_all' && policy.mode !== 'deny_all' && (
        <div className="flex items-center gap-2">
          <dt className="text-xs font-medium text-gray-500 dark:text-gray-400 w-24 shrink-0">
            Tools
          </dt>
          <dd className="text-gray-400 dark:text-gray-500 italic">None configured</dd>
        </div>
      )}
    </dl>
  )
}

export default PolicyDisplay
