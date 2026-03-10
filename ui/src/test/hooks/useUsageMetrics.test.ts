/**
 * Tests for useUsageMetrics hook — aggregation, cache ratio, partial failures,
 * auto-refresh, and computeAggregate helper.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { computeAggregate } from '@/hooks/useUsageMetrics'
import { USAGE_ENTRY_ALPHA, USAGE_ENTRY_BETA } from '@/test/fixtures/usage'
import type { AgentUsageEntry } from '@/hooks/useUsageMetrics'

// ---------------------------------------------------------------------------
// Mock orchestratorClient
// ---------------------------------------------------------------------------

const mockListAgents = vi.fn()
const mockGetAgentUsage = vi.fn()

vi.mock('@/services/orchestrator', () => ({
  orchestratorClient: {
    listAgents: (...args: unknown[]) => mockListAgents(...args),
    getAgentUsage: (...args: unknown[]) => mockGetAgentUsage(...args),
  },
}))

// Mock eventBus to avoid side-effects
vi.mock('@/services/eventBus', () => ({
  agentEventBus: {
    on: () => () => {},
    emit: () => {},
  },
}))

beforeEach(() => {
  vi.useFakeTimers()
  mockListAgents.mockReset()
  mockGetAgentUsage.mockReset()
})

afterEach(() => {
  vi.useRealTimers()
})

// ---------------------------------------------------------------------------
// computeAggregate (pure function) tests
// ---------------------------------------------------------------------------

describe('computeAggregate', () => {
  it('returns zeros for empty entries', () => {
    const result = computeAggregate([])
    expect(result.totalInputTokens).toBe(0)
    expect(result.totalOutputTokens).toBe(0)
    expect(result.totalCostUsd).toBe(0)
    expect(result.totalTokens).toBe(0)
    expect(result.cacheHitRatio).toBe(0)
  })

  it('aggregates tokens across multiple entries', () => {
    const entries: AgentUsageEntry[] = [USAGE_ENTRY_ALPHA, USAGE_ENTRY_BETA]
    const result = computeAggregate(entries)

    // Alpha: input=2000, output=1000, cacheRead=600, cacheCreation=100
    // Beta:  input=1000, output=500,  cacheRead=200, cacheCreation=50
    expect(result.totalInputTokens).toBe(3000)
    expect(result.totalOutputTokens).toBe(1500)
    expect(result.totalCacheReadTokens).toBe(800)
    expect(result.totalCacheCreationTokens).toBe(150)
  })

  it('sums cost across entries', () => {
    const entries: AgentUsageEntry[] = [USAGE_ENTRY_ALPHA, USAGE_ENTRY_BETA]
    const result = computeAggregate(entries)

    // Alpha: 0.04, Beta: 0.02
    expect(result.totalCostUsd).toBeCloseTo(0.06, 6)
  })

  it('computes correct cache hit ratio', () => {
    const entries: AgentUsageEntry[] = [USAGE_ENTRY_ALPHA, USAGE_ENTRY_BETA]
    const result = computeAggregate(entries)

    // cacheRead=800, cacheCreation=150, input=3000
    // ratio = 800 / (800 + 150 + 3000) = 800 / 3950
    expect(result.cacheHitRatio).toBeCloseTo(800 / 3950, 4)
  })

  it('computes totalTokens as sum of all token types', () => {
    const entries: AgentUsageEntry[] = [USAGE_ENTRY_ALPHA]
    const result = computeAggregate(entries)

    // input=2000, output=1000, cacheRead=600, cacheCreation=100
    expect(result.totalTokens).toBe(2000 + 1000 + 600 + 100)
  })

  it('handles single entry', () => {
    const result = computeAggregate([USAGE_ENTRY_ALPHA])
    expect(result.totalInputTokens).toBe(2000)
    expect(result.totalCostUsd).toBe(0.04)
  })
})

// ---------------------------------------------------------------------------
// useUsageMetrics hook tests
// ---------------------------------------------------------------------------

describe('useUsageMetrics hook', () => {
  it('fetches data on mount and populates entries', async () => {
    vi.useRealTimers()

    mockListAgents.mockResolvedValue({
      items: [
        { id: 'a1', name: 'Agent One', status: 'Running' },
        { id: 'a2', name: 'Agent Two', status: 'Running' },
      ],
      total: 2,
    })
    mockGetAgentUsage
      .mockResolvedValueOnce(USAGE_ENTRY_ALPHA.stats)
      .mockResolvedValueOnce(USAGE_ENTRY_BETA.stats)

    // Import dynamically to allow mock setup
    const { useUsageMetrics } = await import('@/hooks/useUsageMetrics')
    const { result } = renderHook(() => useUsageMetrics(10_000))

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.entries).toHaveLength(2)
    expect(result.current.entries[0].agentId).toBe('a1')
    expect(result.current.entries[1].agentId).toBe('a2')
    expect(result.current.aggregate.totalInputTokens).toBeGreaterThan(0)
  })

  it('handles partial failures gracefully', async () => {
    vi.useRealTimers()

    mockListAgents.mockResolvedValue({
      items: [
        { id: 'a1', name: 'Agent One', status: 'Running' },
        { id: 'a2', name: 'Agent Fail', status: 'Running' },
      ],
      total: 2,
    })
    mockGetAgentUsage
      .mockResolvedValueOnce(USAGE_ENTRY_ALPHA.stats)
      .mockRejectedValueOnce(new Error('Network error'))

    const { useUsageMetrics } = await import('@/hooks/useUsageMetrics')
    const { result } = renderHook(() => useUsageMetrics(10_000))

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    // Only the successful agent should appear
    expect(result.current.entries).toHaveLength(1)
    expect(result.current.entries[0].agentId).toBe('a1')
    expect(result.current.error).toBeUndefined()
  })

  it('sets error when listAgents fails', async () => {
    vi.useRealTimers()

    mockListAgents.mockRejectedValue(new Error('Server down'))

    const { useUsageMetrics } = await import('@/hooks/useUsageMetrics')
    const { result } = renderHook(() => useUsageMetrics(10_000))

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.error).toBe('Server down')
    expect(result.current.entries).toHaveLength(0)
  })
})
