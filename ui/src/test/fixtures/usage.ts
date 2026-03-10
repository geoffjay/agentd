/**
 * Shared test fixtures for usage tracking data.
 *
 * Usage:
 *   import { USAGE_STATS, SESSION_USAGE, USAGE_SNAPSHOT } from '@/test/fixtures/usage'
 */

import type {
  AgentUsageStats,
  SessionUsage,
  UsageSnapshot,
  ClearContextResponse,
} from '@/types/orchestrator'
import type { AgentUsageEntry, AggregateUsage } from '@/hooks/useUsageMetrics'

// ---------------------------------------------------------------------------
// SessionUsage fixtures
// ---------------------------------------------------------------------------

export const SESSION_USAGE: SessionUsage = {
  input_tokens: 1500,
  output_tokens: 800,
  cache_read_input_tokens: 400,
  cache_creation_input_tokens: 100,
  total_cost_usd: 0.025,
  num_turns: 5,
  duration_ms: 12_000,
  duration_api_ms: 8_500,
  result_count: 5,
  started_at: '2024-06-15T10:00:00Z',
}

export const EMPTY_SESSION: SessionUsage = {
  input_tokens: 0,
  output_tokens: 0,
  cache_read_input_tokens: 0,
  cache_creation_input_tokens: 0,
  total_cost_usd: 0,
  num_turns: 0,
  duration_ms: 0,
  duration_api_ms: 0,
  result_count: 0,
  started_at: '2024-06-15T12:00:00Z',
}

// ---------------------------------------------------------------------------
// AgentUsageStats fixtures
// ---------------------------------------------------------------------------

export const USAGE_STATS: AgentUsageStats = {
  agent_id: 'agent-usage-1',
  current_session: { ...SESSION_USAGE },
  cumulative: {
    input_tokens: 5000,
    output_tokens: 2500,
    cache_read_input_tokens: 1200,
    cache_creation_input_tokens: 300,
    total_cost_usd: 0.08,
    num_turns: 15,
    duration_ms: 45_000,
    duration_api_ms: 32_000,
    result_count: 15,
    started_at: '2024-06-10T08:00:00Z',
  },
  session_count: 3,
}

export const USAGE_STATS_NO_SESSION: AgentUsageStats = {
  agent_id: 'agent-usage-2',
  cumulative: {
    input_tokens: 2000,
    output_tokens: 1000,
    cache_read_input_tokens: 500,
    cache_creation_input_tokens: 100,
    total_cost_usd: 0.03,
    num_turns: 8,
    duration_ms: 20_000,
    duration_api_ms: 15_000,
    result_count: 8,
    started_at: '2024-06-12T09:00:00Z',
  },
  session_count: 2,
}

export const USAGE_STATS_ZERO: AgentUsageStats = {
  agent_id: 'agent-usage-zero',
  current_session: { ...EMPTY_SESSION },
  cumulative: {
    input_tokens: 0,
    output_tokens: 0,
    cache_read_input_tokens: 0,
    cache_creation_input_tokens: 0,
    total_cost_usd: 0,
    num_turns: 0,
    duration_ms: 0,
    duration_api_ms: 0,
    result_count: 0,
    started_at: '2024-06-15T12:00:00Z',
  },
  session_count: 0,
}

// ---------------------------------------------------------------------------
// UsageSnapshot fixture (single-turn delta)
// ---------------------------------------------------------------------------

export const USAGE_SNAPSHOT: UsageSnapshot = {
  input_tokens: 100,
  output_tokens: 50,
  cache_read_input_tokens: 20,
  cache_creation_input_tokens: 10,
  total_cost_usd: 0.005,
  num_turns: 1,
  duration_ms: 3000,
  duration_api_ms: 2200,
}

// ---------------------------------------------------------------------------
// ClearContextResponse fixture
// ---------------------------------------------------------------------------

export const CLEAR_CONTEXT_RESPONSE: ClearContextResponse = {
  agent_id: 'agent-usage-1',
  new_session_number: 4,
  session_usage: { ...SESSION_USAGE },
}

// ---------------------------------------------------------------------------
// AgentUsageEntry fixtures (for useUsageMetrics)
// ---------------------------------------------------------------------------

export const USAGE_ENTRY_ALPHA: AgentUsageEntry = {
  agentId: 'agent-alpha',
  name: 'Agent Alpha',
  stats: {
    agent_id: 'agent-alpha',
    current_session: {
      input_tokens: 500,
      output_tokens: 250,
      cache_read_input_tokens: 100,
      cache_creation_input_tokens: 30,
      total_cost_usd: 0.01,
      num_turns: 3,
      duration_ms: 6000,
      duration_api_ms: 4000,
      result_count: 3,
      started_at: '2024-06-15T10:00:00Z',
    },
    cumulative: {
      input_tokens: 2000,
      output_tokens: 1000,
      cache_read_input_tokens: 600,
      cache_creation_input_tokens: 100,
      total_cost_usd: 0.04,
      num_turns: 10,
      duration_ms: 25000,
      duration_api_ms: 18000,
      result_count: 10,
      started_at: '2024-06-10T08:00:00Z',
    },
    session_count: 2,
  },
}

export const USAGE_ENTRY_BETA: AgentUsageEntry = {
  agentId: 'agent-beta',
  name: 'Agent Beta',
  stats: {
    agent_id: 'agent-beta',
    current_session: {
      input_tokens: 300,
      output_tokens: 150,
      cache_read_input_tokens: 50,
      cache_creation_input_tokens: 20,
      total_cost_usd: 0.005,
      num_turns: 2,
      duration_ms: 4000,
      duration_api_ms: 3000,
      result_count: 2,
      started_at: '2024-06-15T11:00:00Z',
    },
    cumulative: {
      input_tokens: 1000,
      output_tokens: 500,
      cache_read_input_tokens: 200,
      cache_creation_input_tokens: 50,
      total_cost_usd: 0.02,
      num_turns: 5,
      duration_ms: 12000,
      duration_api_ms: 9000,
      result_count: 5,
      started_at: '2024-06-11T09:00:00Z',
    },
    session_count: 1,
  },
}

// ---------------------------------------------------------------------------
// AggregateUsage fixture
// ---------------------------------------------------------------------------

export const AGGREGATE_USAGE: AggregateUsage = {
  totalInputTokens: 3000,
  totalOutputTokens: 1500,
  totalCacheReadTokens: 800,
  totalCacheCreationTokens: 150,
  totalCostUsd: 0.06,
  totalTokens: 5450,
  // cacheHitRatio = 800 / (800 + 150 + 3000) = 800 / 3950 ≈ 0.2025
  cacheHitRatio: 800 / (800 + 150 + 3000),
}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

export function makeSessionUsage(overrides?: Partial<SessionUsage>): SessionUsage {
  return { ...SESSION_USAGE, ...overrides }
}

export function makeUsageStats(overrides?: Partial<AgentUsageStats>): AgentUsageStats {
  return { ...USAGE_STATS, ...overrides }
}

export function makeUsageEntry(
  overrides?: Partial<AgentUsageEntry>,
): AgentUsageEntry {
  return { ...USAGE_ENTRY_ALPHA, ...overrides }
}
