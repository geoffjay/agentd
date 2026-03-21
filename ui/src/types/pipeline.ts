/**
 * Pipeline types for the v0.10.0 Autonomous Development Pipeline.
 *
 * The Conductor agent manages the merge queue and git-spice stack state.
 * These types represent the data shape exposed to the UI; the backend
 * endpoint that produces this data is defined in the v0.10.0 conductor
 * implementation (issues #603 / #604).
 *
 * Until the endpoint exists, usePipelineStatus() returns null and the
 * PipelineStatusCard renders its "not yet active" empty state.
 */

/** A single PR waiting in the merge queue */
export interface PipelineQueueItem {
  /** GitHub PR number */
  prNumber: number
  /** PR title (truncated for display) */
  title: string
  /** Head branch name */
  branch: string
  /** Base branch (trunk = "main", or parent stack branch) */
  baseBranch: string
  /**
   * Stack depth: 0 = branches directly from trunk (main),
   * N = Nth level in a git-spice stack.
   */
  stackDepth: number
  /** Approved by the reviewer agent */
  approved: boolean
  /** CI status. null = checks still running */
  ciPassing: boolean | null
}

/** Aggregate pipeline state posted by the Conductor every 4 hours */
export interface PipelineStatus {
  /** PRs in the merge queue, ordered bottom-of-stack first */
  mergeQueue: PipelineQueueItem[]
  /** Number of distinct open stacks (groups of stacked branches) */
  activeStackCount: number
  /** PRs with no activity (commits or review comments) for > 7 days */
  staleCount: number
  /** ISO timestamp of the last `git-spice repo sync` run */
  lastSyncAt: string | null
  /** ISO timestamp of the last full conductor run */
  conductorLastRunAt: string | null
}

/**
 * Human interaction gate definitions.
 *
 * Sourced from the v0.10.0 milestone spec (issue #611).
 * Always-human gates are hardcoded in the conductor system prompt and cannot
 * be changed via the UI. Configurable gates default to autonomous but can
 * be flipped per-project in the agent YAML.
 */
export interface PipelineGate {
  id: string
  label: string
  description: string
  /** 'always' = hardcoded human required; 'configurable' = default autonomous */
  kind: 'always' | 'configurable'
  /** Default state for configurable gates */
  defaultAutonomous?: boolean
}

export const PIPELINE_GATES: PipelineGate[] = [
  // Always-human gates
  {
    id: 'git-spice-auth',
    label: 'git-spice authentication',
    description: 'One-time GitHub auth for git-spice must be performed by a human operator.',
    kind: 'always',
  },
  {
    id: 'production-deploy',
    label: 'Production deployments',
    description: 'Any deployment to a production environment requires human sign-off.',
    kind: 'always',
  },
  {
    id: 'security-changes',
    label: 'Security-sensitive changes',
    description:
      'Changes to authentication, secrets handling, or cryptography require human review.',
    kind: 'always',
  },
  {
    id: 'conflict-escalation',
    label: 'Merge conflict escalation',
    description:
      'When the Conductor cannot automatically restack a conflict, it escalates to a human.',
    kind: 'always',
  },
  // Configurable gates
  {
    id: 'pr-auto-merge',
    label: 'PR auto-merge',
    description: 'Conductor merges approved, CI-passing PRs without human confirmation.',
    kind: 'configurable',
    defaultAutonomous: true,
  },
  {
    id: 'issue-auto-close',
    label: 'Issue auto-close',
    description: 'Issues are automatically closed when their implementation PR merges.',
    kind: 'configurable',
    defaultAutonomous: true,
  },
  {
    id: 'new-dependency',
    label: 'New dependency additions',
    description: 'Adding a new crate or package dependency requires human approval.',
    kind: 'configurable',
    defaultAutonomous: false,
  },
]
