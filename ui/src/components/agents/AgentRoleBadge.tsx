/**
 * AgentRoleBadge — visual identifier for the functional role of an agent.
 *
 * Each of the ten agent roles (5 original + 5 v0.9.0 expansion) receives a
 * distinct colour drawn from Tailwind's semantic palette and a lucide-react
 * icon chosen to reflect the role's purpose.
 *
 * Variants:
 *   'badge'     — coloured pill with icon + text label (default)
 *   'dot'       — small coloured circle only (for very compact contexts)
 *   'icon-only' — icon with accessible aria-label (for table cells)
 *
 * Colour design decisions
 * ───────────────────────
 * All badge colours use the same pill structure as StatusBadge:
 *   bg-{colour}-100 / text-{colour}-800 (light)
 *   dark:bg-{colour}-900/30 / dark:text-{colour}-400
 * This keeps contrast ratios consistent and integrates with the existing
 * colour system without introducing new design tokens.
 *
 * Role → colour mapping rationale:
 *   planner    → blue     (strategic, directional thinking)
 *   worker     → emerald  (execution, active doing)
 *   reviewer   → amber    (scrutiny, caution, inspection)
 *   documenter → violet   (knowledge, writing, structure)
 *   designer   → pink     (visual creativity)
 *   refactor   → teal     (transformation, code cleanliness)
 *   research   → indigo   (deep investigation)
 *   enricher   → sky      (augmentation, expanding context)
 *   tester     → lime     (quality gates, green = pass)
 *   auditor    → rose     (security, vigilance, risk)
 *   unknown    → gray     (neutral fallback)
 */

import {
  BookOpen,
  Code2,
  FlaskConical,
  Map,
  Palette,
  Search,
  ShieldAlert,
  Sparkles,
  TestTube2,
  Wrench,
} from 'lucide-react'
import type { LucideProps } from 'lucide-react'
import type { AgentRole } from '@/types/agent-roles'
import { inferAgentRole, ROLE_LABELS } from '@/types/agent-roles'

// ---------------------------------------------------------------------------
// Style maps
// ---------------------------------------------------------------------------

const BADGE_STYLES: Record<AgentRole, string> = {
  planner:
    'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
  worker:
    'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-400',
  reviewer:
    'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400',
  documenter:
    'bg-violet-100 text-violet-800 dark:bg-violet-900/30 dark:text-violet-400',
  designer:
    'bg-pink-100 text-pink-800 dark:bg-pink-900/30 dark:text-pink-400',
  refactor:
    'bg-teal-100 text-teal-800 dark:bg-teal-900/30 dark:text-teal-400',
  research:
    'bg-indigo-100 text-indigo-800 dark:bg-indigo-900/30 dark:text-indigo-400',
  enricher:
    'bg-sky-100 text-sky-800 dark:bg-sky-900/30 dark:text-sky-400',
  tester:
    'bg-lime-100 text-lime-800 dark:bg-lime-900/30 dark:text-lime-400',
  auditor:
    'bg-rose-100 text-rose-800 dark:bg-rose-900/30 dark:text-rose-400',
  unknown:
    'bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-400',
}

const DOT_STYLES: Record<AgentRole, string> = {
  planner: 'bg-blue-500',
  worker: 'bg-emerald-500',
  reviewer: 'bg-amber-500',
  documenter: 'bg-violet-500',
  designer: 'bg-pink-500',
  refactor: 'bg-teal-500',
  research: 'bg-indigo-500',
  enricher: 'bg-sky-500',
  tester: 'bg-lime-500',
  auditor: 'bg-rose-500',
  unknown: 'bg-gray-400',
}

// ---------------------------------------------------------------------------
// Icon map
// ---------------------------------------------------------------------------

type IconComponent = React.ComponentType<LucideProps>

const ROLE_ICONS: Record<AgentRole, IconComponent> = {
  planner: Map,
  worker: Wrench,
  reviewer: Search,
  documenter: BookOpen,
  designer: Palette,
  refactor: Code2,
  research: FlaskConical,
  enricher: Sparkles,
  tester: TestTube2,
  auditor: ShieldAlert,
  unknown: Wrench,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface AgentRoleBadgeProps {
  /**
   * Pass either a pre-resolved AgentRole or the raw agent name string.
   * When a string that is not a known role is passed, `inferAgentRole` is
   * applied to derive the role from the name automatically.
   */
  role: AgentRole | string
  variant?: 'badge' | 'dot' | 'icon-only'
  className?: string
}

export function AgentRoleBadge({
  role: roleProp,
  variant = 'badge',
  className = '',
}: AgentRoleBadgeProps) {
  // Resolve role — accept either a pre-typed AgentRole or a raw name string
  const role: AgentRole =
    ROLE_LABELS[roleProp as AgentRole] !== undefined
      ? (roleProp as AgentRole)
      : inferAgentRole(roleProp)

  const label = ROLE_LABELS[role]
  const Icon = ROLE_ICONS[role]

  if (variant === 'dot') {
    return (
      <span
        role="img"
        aria-label={label}
        title={label}
        className={[
          'inline-block h-2.5 w-2.5 rounded-full',
          DOT_STYLES[role],
          className,
        ].join(' ')}
      />
    )
  }

  if (variant === 'icon-only') {
    return (
      <span
        role="img"
        aria-label={label}
        title={label}
        className={['inline-flex items-center', className].join(' ')}
      >
        <Icon
          size={14}
          aria-hidden="true"
          className={
            // Re-use the text colour portion of the badge style
            BADGE_STYLES[role].split(' ').find((c) => c.startsWith('text-')) ??
            'text-gray-500'
          }
        />
      </span>
    )
  }

  // Default: 'badge'
  return (
    <span
      role="img"
      aria-label={label}
      className={[
        'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
        BADGE_STYLES[role],
        className,
      ].join(' ')}
    >
      <Icon size={11} aria-hidden="true" />
      {label}
    </span>
  )
}

export default AgentRoleBadge
