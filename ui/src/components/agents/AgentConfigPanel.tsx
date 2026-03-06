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
 */

import { useState } from 'react'
import { ChevronDown, ChevronRight, Eye, EyeOff } from 'lucide-react'
import type { Agent, ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Tool policy display helper
// ---------------------------------------------------------------------------

function policyLabel(policy: ToolPolicy): string {
  switch (policy.type) {
    case 'AllowAll':
      return 'Allow All tools'
    case 'DenyAll':
      return 'Deny All tools'
    case 'RequireApproval':
      return 'Require Approval for all tools'
    case 'AllowList':
      return `Allow: ${policy.tools.length > 0 ? policy.tools.join(', ') : '(none)'}`
    case 'DenyList':
      return `Deny: ${policy.tools.length > 0 ? policy.tools.join(', ') : '(none)'}`
  }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ConfigRow({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
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
        <span className="text-xs font-medium text-gray-400 dark:text-gray-500">
          Environment
        </span>
        <button
          type="button"
          aria-label={revealed ? 'Hide env values' : 'Show env values'}
          onClick={() => setRevealed(v => !v)}
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
  const display = !expanded && isLong ? `${prompt.slice(0, 200)}…` : prompt

  return (
    <div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
      <span className="w-36 flex-shrink-0 text-xs font-medium text-gray-400 dark:text-gray-500">
        System Prompt
      </span>
      <div className="flex flex-col gap-1">
        <p className="whitespace-pre-wrap text-sm text-gray-700 dark:text-gray-300">
          {display}
        </p>
        {isLong && (
          <button
            type="button"
            onClick={() => setExpanded(e => !e)}
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
// AgentConfigPanel
// ---------------------------------------------------------------------------

export interface AgentConfigPanelProps {
  agent: Agent
}

export function AgentConfigPanel({ agent }: AgentConfigPanelProps) {
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
        onClick={() => setOpen(o => !o)}
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

          {agent.tmux_session && (
            <ConfigRow label="Tmux Session">
              <span className="font-mono text-xs">{agent.tmux_session}</span>
            </ConfigRow>
          )}

          <ConfigRow label="Tool Policy">
            {policyLabel(config.tool_policy)}
          </ConfigRow>

          {config.system_prompt && (
            <SystemPromptRow prompt={config.system_prompt} />
          )}

          {config.env && Object.keys(config.env).length > 0 && (
            <EnvVarsRow env={config.env} />
          )}
        </div>
      )}
    </section>
  )
}

export default AgentConfigPanel
