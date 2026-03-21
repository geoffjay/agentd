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
 * Milestone #19 (Specialized Agent Ecosystem) is the authoritative source,
 * confirmed by group analysis. The 8 agent names from #19 are the canonical
 * identifiers that will appear in .agentd/ YAML files and GitHub labels.
 *
 * Aliases handle variant names from earlier planning iterations (#17, #18):
 *   researcher    → research   (#17 used "researcher")
 *   auditor       → security   (#18 used "auditor"; #19 GitHub label is "security")
 *   test-writer   → tester     (#17 used "test-writer"; display label kept as "Tester")
 *   test          → tester     (#19 agent name; display label kept as "Tester" for clarity)
 *   release-mgr   → release    (#17 used "release-manager")
 *   issue-quality → enricher   (#19 agent name; TS-friendly canonical kept as "enricher")
 *
 * Note: 'enricher' and 'tester' are kept as canonical TypeScript names (not
 * 'issue-quality' / 'test') because they lack hyphens and produce clearer
 * badge labels. The YAML agent names 'issue-quality' and 'test' resolve to
 * them via inferAgentRole().
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
  | 'enricher'    // issue quality improvement; aliases: issue-quality (#19 YAML name)
  | 'tester'      // test coverage; aliases: test (#19 YAML name), test-writer (#17)
  | 'security'    // dependency auditing, CVE triage; alias: auditor (#18)
  | 'release'     // changelog, versioning, releases; alias: release-manager (#17)
  | 'unknown'

// ---------------------------------------------------------------------------
// Canonical role lookup
// ---------------------------------------------------------------------------

const KNOWN_ROLES = new Set<string>([
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  'architect', 'refactor', 'research', 'triage',
  'enricher', 'tester', 'security', 'release',
])

/**
 * Maps non-canonical names to their canonical AgentRole.
 * Sources: milestone #17 (Ecosystem Expansion), #18 (Workforce Expansion),
 * #19 (Specialized Agent Ecosystem).
 */
const ROLE_ALIASES: Record<string, AgentRole> = {
  // #19 YAML agent names that differ from the canonical TS identifier
  'issue-quality':   'enricher',    // #19 agent is named "issue-quality"
  test:              'tester',      // #19 agent is named "test"
  // Earlier planning iterations
  researcher:        'research',    // #17 used "researcher"
  'test-writer':     'tester',      // #17 used "test-writer"
  'release-manager': 'release',     // #17 used "release-manager"
  auditor:           'security',    // #18 used "auditor"
}

/**
 * Infer an agent's functional role from its display name.
 *
 * Handles naming conventions used in .agentd/ YAML templates:
 *   "planner"           → planner
 *   "refactor-agent"    → refactor
 *   "tester-agent-1"    → tester
 *   "researcher"        → research   (alias)
 *   "auditor-agent"     → security   (alias)
 *   "issue-quality"     → enricher   (alias)
 *   "test"              → tester     (alias)
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
  security:   'Security',
  release:    'Release',
  unknown:    'Unknown',
}

/** All known non-unknown roles, ordered for display in filter dropdowns */
export const ALL_AGENT_ROLES: Exclude<AgentRole, 'unknown'>[] = [
  // Original workforce
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  // v0.9.0 expansion
  'architect', 'refactor', 'research', 'triage',
  'enricher', 'tester', 'security', 'release',
]
