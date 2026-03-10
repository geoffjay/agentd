/**
 * Extended tests for useAgentUsage hook — auto-refresh, clearContext action,
 * error handling, and cleanup.
 *
 * Complements the existing useAgentUsage.test.ts which covers real-time events.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { renderHook, act, waitFor } from '@testing-library/react'
import { useAgentUsage } from '@/hooks/useAgentUsage'
import { USAGE_STATS } from '@/test/fixtures/usage'

// ---------------------------------------------------------------------------
// Mock orchestratorClient
// ---------------------------------------------------------------------------

const mockGetAgentUsage = vi.fn()
const mockClearContext = vi.fn()

vi.mock('@/services/orchestrator', () => ({
  orchestratorClient: {
    getAgentUsage: (...args: unknown[]) => mockGetAgentUsage(...args),
    clearContext: (...args: unknown[]) => mockClearContext(...args),
  },
}))

beforeEach(() => {
  mockGetAgentUsage.mockReset().mockResolvedValue({ ...USAGE_STATS })
  mockClearContext.mockReset().mockResolvedValue({
    agent_id: 'agent-1',
    new_session_number: 4,
  })
})

afterEach(() => {
  vi.useRealTimers()
})

describe('useAgentUsage extended', () => {
  it('fetches usage on mount and sets loading states', async () => {
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    // Initially loading
    expect(result.current.loading).toBe(true)

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.usage).not.toBeNull()
    expect(result.current.usage?.agent_id).toBe('agent-usage-1')
    expect(result.current.error).toBeNull()
    expect(mockGetAgentUsage).toHaveBeenCalledWith('agent-1')
  })

  it('handles API errors gracefully', async () => {
    mockGetAgentUsage.mockRejectedValue(new Error('Network timeout'))

    const { result } = renderHook(() => useAgentUsage('agent-err', 0))

    await waitFor(() => {
      expect(result.current.loading).toBe(false)
    })

    expect(result.current.usage).toBeNull()
    expect(result.current.error).toBe('Network timeout')
  })

  it('auto-refreshes on configured interval', async () => {
    vi.useFakeTimers()

    renderHook(() => useAgentUsage('agent-1', 5000))

    // Initial fetch
    await vi.advanceTimersByTimeAsync(0)
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)

    // Advance to first refresh
    await vi.advanceTimersByTimeAsync(5000)
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(2)

    // Advance to second refresh
    await vi.advanceTimersByTimeAsync(5000)
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(3)
  })

  it('does not auto-refresh when interval is 0', async () => {
    vi.useFakeTimers()

    renderHook(() => useAgentUsage('agent-1', 0))

    await vi.advanceTimersByTimeAsync(0)
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)

    await vi.advanceTimersByTimeAsync(30000)
    // Should still be 1 — no auto-refresh
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)
  })

  it('clearContext calls API and refreshes usage data', async () => {
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    expect(result.current.clearing).toBe(false)

    await act(async () => {
      const response = await result.current.clearContext()
      expect(response.new_session_number).toBe(4)
    })

    expect(mockClearContext).toHaveBeenCalledWith('agent-1')
    // clearContext triggers a refresh, so getAgentUsage called again
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(2)
  })

  it('clearing state toggles during clearContext', async () => {
    // Make clearContext slow
    mockClearContext.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve({ agent_id: 'agent-1', new_session_number: 4 }), 100)),
    )

    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    // Start clearing (don't await yet)
    let clearPromise: Promise<unknown>
    act(() => {
      clearPromise = result.current.clearContext()
    })

    // Should be clearing
    expect(result.current.clearing).toBe(true)

    await act(async () => {
      await clearPromise!
    })

    expect(result.current.clearing).toBe(false)
  })

  it('cleans up interval on unmount', async () => {
    vi.useFakeTimers()

    const { unmount } = renderHook(() => useAgentUsage('agent-1', 5000))

    await vi.advanceTimersByTimeAsync(0)
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)

    unmount()

    // Advance past when the next refresh would fire
    await vi.advanceTimersByTimeAsync(10000)
    // Should still only be 1 call — interval was cleaned up
    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)
  })

  it('refresh method triggers a manual fetch', async () => {
    const { result } = renderHook(() => useAgentUsage('agent-1', 0))

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    expect(mockGetAgentUsage).toHaveBeenCalledTimes(1)

    act(() => {
      result.current.refresh()
    })

    await waitFor(() => {
      expect(mockGetAgentUsage).toHaveBeenCalledTimes(2)
    })
  })

  it('re-fetches when agentId changes', async () => {
    const { result, rerender } = renderHook(
      ({ id }: { id: string }) => useAgentUsage(id, 0),
      { initialProps: { id: 'agent-1' } },
    )

    await waitFor(() => {
      expect(result.current.usage).not.toBeNull()
    })

    expect(mockGetAgentUsage).toHaveBeenCalledWith('agent-1')

    rerender({ id: 'agent-2' })

    await waitFor(() => {
      expect(mockGetAgentUsage).toHaveBeenCalledWith('agent-2')
    })
  })
})
