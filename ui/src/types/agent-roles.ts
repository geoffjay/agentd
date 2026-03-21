/**
 * Agent role type definitions and inference utilities.
 *
 * Until the orchestrator exposes a dedicated `role` field on Agent (v0.9.0
 * backend work), the role is inferred from the agent name by stripping common
 * suffixes such as "-agent" and matching against the known role set.
 *
 * When the backend adds `role` to AgentConfig or Agent, this inference
 * function should be deprecated in favour of reading the field directly.
 */

export type AgentRole =
  // Original workforce
  | 'planner'
  | 'worker'
  | 'reviewer'
  | 'documenter'
  | 'designer'
  // v0.9.0 Agent Workforce Expansion
  | 'refactor'
  | 'research'
  | 'enricher'
  | 'tester'
  | 'auditor'
  | 'unknown'

const KNOWN_ROLES = new Set<string>([
  'planner',
  'worker',
  'reviewer',
  'documenter',
  'designer',
  'refactor',
  'research',
  'enricher',
  'tester',
  'auditor',
])

/**
 * Infer an agent's functional role from its display name.
 *
 * Handles naming conventions used in .agentd/ YAML templates:
 *   "planner"         → planner
 *   "refactor-agent"  → refactor
 *   "tester-agent-1"  → tester
 *   "my-custom-bot"   → unknown
 */
export function inferAgentRole(name: string): AgentRole {
  const normalized = name
    .toLowerCase()
    .replace(/-agent$/i, '')   // strip "-agent" suffix
    .replace(/[-_]\d+$/, '')   // strip trailing index (e.g. -1, _2)
    .trim()

  return KNOWN_ROLES.has(normalized) ? (normalized as AgentRole) : 'unknown'
}

/** Human-readable label for each role */
export const ROLE_LABELS: Record<AgentRole, string> = {
  planner: 'Planner',
  worker: 'Worker',
  reviewer: 'Reviewer',
  documenter: 'Documenter',
  designer: 'Designer',
  refactor: 'Refactor',
  research: 'Research',
  enricher: 'Enricher',
  tester: 'Tester',
  auditor: 'Auditor',
  unknown: 'Unknown',
}

/** All known non-unknown roles, ordered for display in filter dropdowns */
export const ALL_AGENT_ROLES: Exclude<AgentRole, 'unknown'>[] = [
  'planner',
  'worker',
  'reviewer',
  'documenter',
  'designer',
  'refactor',
  'research',
  'enricher',
  'tester',
  'auditor',
]
