/**
 * Agent role type definitions and inference utilities.
 *
 * Until the orchestrator exposes a dedicated `role` field on Agent (v0.9.0
 * backend work), the role is inferred from the agent name by stripping common
 * suffixes such as "-agent" and matching against the known role set.
 *
 * When the backend adds `role` to AgentConfig or Agent, this inference
 * function should be deprecated in favour of reading the field directly.
 *
 * Canonical role set
 * ──────────────────
 * Derived from the union of three v0.9.0 milestone plans (#17, #18, #19).
 * Where the plans used different names for the same concept, a canonical name
 * was chosen and the others are handled as aliases in inferAgentRole():
 *
 *   researcher   → research   (#17 used "researcher", #18/#19 used "research")
 *   security     → auditor    (#17/#19 used "security", #18 used "auditor")
 *   test-writer  → tester     (#17 used "test-writer", #18 used "tester")
 *   test         → tester     (#19 used "test")
 *   release-mgr  → release    (#17 used "release-manager", #19 used "release")
 *   issue-quality→ enricher   (#19 used "issue-quality", #18 used "enricher")
 *
 * ⚠ Milestone naming collision: milestones #17, #18, and #19 are all tagged
 * v0.9.0. The UI treats them as a unified set until the backend resolves the
 * version conflict.
 */

export type AgentRole =
  // Original workforce (v0.0.x–v0.8.x)
  | 'planner'
  | 'worker'
  | 'reviewer'
  | 'documenter'
  | 'designer'
  // v0.9.0 unified canonical set (milestones #17 + #18 + #19)
  | 'architect'   // cross-service design review, ADRs (#17, #19)
  | 'refactor'    // targeted code improvements (#18, #19)
  | 'research'    // technology investigation (#18, #19; alias: researcher)
  | 'triage'      // issue labeling and enrichment (#17, #19)
  | 'enricher'    // issue quality improvement (#18; alias: issue-quality)
  | 'tester'      // test coverage (#18; aliases: test-writer, test)
  | 'auditor'     // security/dependency auditing (#18; alias: security)
  | 'release'     // changelog, versioning, releases (#17, #19; alias: release-manager)
  | 'unknown'

// ---------------------------------------------------------------------------
// Canonical role lookup
// ---------------------------------------------------------------------------

const KNOWN_ROLES = new Set<string>([
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  'architect', 'refactor', 'research', 'triage',
  'enricher', 'tester', 'auditor', 'release',
])

/**
 * Maps non-canonical names to their canonical AgentRole.
 * Sources: milestone #17 (Ecosystem Expansion), #18 (Workforce Expansion),
 * #19 (Specialized Agent Ecosystem).
 */
const ROLE_ALIASES: Record<string, AgentRole> = {
  // #17 uses longer, explicit names
  researcher:        'research',
  'test-writer':     'tester',
  'release-manager': 'release',
  // #17 and #19 use "security" for what #18 calls "auditor"
  security:          'auditor',
  // #19 uses terse single-word forms
  test:              'tester',
  'issue-quality':   'enricher',
}

/**
 * Infer an agent's functional role from its display name.
 *
 * Handles naming conventions used in .agentd/ YAML templates:
 *   "planner"           → planner
 *   "refactor-agent"    → refactor
 *   "tester-agent-1"    → tester
 *   "researcher"        → research   (alias)
 *   "security-agent"    → auditor    (alias)
 *   "test-writer-agent" → tester     (alias)
 *   "release-manager"   → release    (alias)
 *   "my-custom-bot"     → unknown
 */
export function inferAgentRole(name: string): AgentRole {
  const normalized = name
    .toLowerCase()
    .replace(/-agent$/i, '')  // strip "-agent" suffix
    .replace(/[-_]\d+$/, '')  // strip trailing index (e.g. -1, _2)
    .trim()

  if (KNOWN_ROLES.has(normalized)) return normalized as AgentRole
  if (normalized in ROLE_ALIASES) return ROLE_ALIASES[normalized]
  return 'unknown'
}

// ---------------------------------------------------------------------------
// Display metadata
// ---------------------------------------------------------------------------

/** Human-readable label for each canonical role */
export const ROLE_LABELS: Record<AgentRole, string> = {
  planner:    'Planner',
  worker:     'Worker',
  reviewer:   'Reviewer',
  documenter: 'Documenter',
  designer:   'Designer',
  architect:  'Architect',
  refactor:   'Refactor',
  research:   'Research',
  triage:     'Triage',
  enricher:   'Enricher',
  tester:     'Tester',
  auditor:    'Auditor',
  release:    'Release',
  unknown:    'Unknown',
}

/** All known non-unknown roles, ordered for display in filter dropdowns */
export const ALL_AGENT_ROLES: Exclude<AgentRole, 'unknown'>[] = [
  // Original workforce
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  // v0.9.0 expansion
  'architect', 'refactor', 'research', 'triage',
  'enricher', 'tester', 'auditor', 'release',
]
