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
 * Milestone #18 (Consolidated Autonomous Development Pipeline) is the
 * authoritative source. The planner superseded milestones #17 and #19;
 * those have been deleted. The 7 new agents from #18 are the canonical
 * identifiers that will appear in .agentd/ YAML files and GitHub labels.
 *
 * Aliases handle variant names from earlier planning iterations:
 *   researcher    → research   (#17 used "researcher")
 *   auditor       → security   (#18 pre-consolidation used "auditor")
 *   test-writer   → tester     (#17 used "test-writer")
 *   test          → tester     (#19 agent name; display label kept as "Tester")
 *   issue-quality → enricher   (#19 agent name; TS-friendly canonical kept as "enricher")
 *   architect     → unknown    (PR #595 closed, never shipped)
 *   release       → unknown    (dropped from consolidated milestone #18)
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
  // v0.9.0 consolidated canonical set (milestone #18)
  | 'conductor'   // pipeline orchestration; merge queue, git-spice restack, escalation
  | 'triage'      // issue labeling and prioritisation
  | 'enricher'    // issue quality improvement; alias: issue-quality
  | 'tester'      // test coverage; aliases: test, test-writer
  | 'refactor'    // targeted code improvements
  | 'research'    // technology investigation; alias: researcher
  | 'security'    // dependency auditing, CVE triage; alias: auditor
  | 'unknown'

// ---------------------------------------------------------------------------
// Canonical role lookup
// ---------------------------------------------------------------------------

const KNOWN_ROLES = new Set<string>([
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  'conductor', 'triage', 'enricher', 'tester',
  'refactor', 'research', 'security',
])

/**
 * Maps non-canonical names to their canonical AgentRole.
 * Sources: milestone #18 consolidation (supersedes #17 and #19).
 */
const ROLE_ALIASES: Record<string, AgentRole> = {
  // YAML agent names that differ from the canonical TS identifier
  'issue-quality':   'enricher',    // #19 agent was named "issue-quality"
  test:              'tester',      // #19 agent was named "test"
  // Earlier planning iterations
  researcher:        'research',    // #17 used "researcher"
  'test-writer':     'tester',      // #17 used "test-writer"
  'release-manager': 'unknown',     // dropped from consolidated #18
  auditor:           'security',    // #18 pre-consolidation used "auditor"
  // Closed/dropped agents
  architect:         'unknown',     // PR #595 closed, never shipped
  release:           'unknown',     // dropped from consolidated #18
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
 *   "architect"         → unknown    (dropped)
 *   "release-manager"   → unknown    (dropped)
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
  conductor:  'Conductor',
  triage:     'Triage',
  enricher:   'Enricher',
  tester:     'Tester',
  refactor:   'Refactor',
  research:   'Research',
  security:   'Security',
  unknown:    'Unknown',
}

/** All known non-unknown roles, ordered for display in filter dropdowns */
export const ALL_AGENT_ROLES: Exclude<AgentRole, 'unknown'>[] = [
  // Original workforce
  'planner', 'worker', 'reviewer', 'documenter', 'designer',
  // v0.9.0 consolidated expansion (milestone #18)
  'conductor', 'triage', 'enricher', 'tester',
  'refactor', 'research', 'security',
]
