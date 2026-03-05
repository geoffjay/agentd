/**
 * Test data factory for Agent and related Orchestrator types.
 *
 * Usage:
 *   const agent = makeAgent()           // default values
 *   const agent = makeAgent({ status: 'Failed' })  // partial override
 *   const agents = makeAgentList(5)     // list of 5 agents
 */

import type { Agent, AgentConfig, PendingApproval } from '@/types/orchestrator'

let _seq = 0
function nextId(): string {
  return String(++_seq)
}

/** Reset the sequence counter (call in beforeEach to get predictable IDs) */
export function resetAgentSeq(): void {
  _seq = 0
}

// ---------------------------------------------------------------------------
// AgentConfig factory
// ---------------------------------------------------------------------------

export function makeAgentConfig(overrides?: Partial<AgentConfig>): AgentConfig {
  return {
    working_dir: '/tmp/agent',
    shell: '/bin/bash',
    interactive: false,
    tool_policy: { type: 'AllowAll' },
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// Agent factory
// ---------------------------------------------------------------------------

export function makeAgent(overrides?: Partial<Agent>): Agent {
  const id = nextId()
  return {
    id,
    name: `test-agent-${id}`,
    status: 'Running',
    config: makeAgentConfig(),
    created_at: '2024-01-01T00:00:00.000Z',
    updated_at: '2024-01-01T00:00:00.000Z',
    ...overrides,
  }
}

/** Create a list of N agents with auto-incrementing IDs */
export function makeAgentList(count: number, overrides?: Partial<Agent>): Agent[] {
  return Array.from({ length: count }, () => makeAgent(overrides))
}

// ---------------------------------------------------------------------------
// PendingApproval factory
// ---------------------------------------------------------------------------

export function makePendingApproval(overrides?: Partial<PendingApproval>): PendingApproval {
  const id = nextId()
  return {
    id,
    agent_id: nextId(),
    request_id: nextId(),
    tool_name: 'bash',
    tool_input: { command: 'ls -la' },
    status: 'Pending',
    created_at: '2024-01-01T00:00:00.000Z',
    expires_at: '2024-01-01T01:00:00Z',
    ...overrides,
  }
}

export function makeApprovalList(
  count: number,
  overrides?: Partial<PendingApproval>,
): PendingApproval[] {
  return Array.from({ length: count }, () => makePendingApproval(overrides))
}
