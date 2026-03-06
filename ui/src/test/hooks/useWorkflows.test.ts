/**
 * Tests for useWorkflows hook.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { useWorkflows, useDispatchHistory } from '@/hooks/useWorkflows'
import { orchestratorClient } from '@/services/orchestrator'
import type { Workflow, DispatchRecord } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeWorkflow(overrides: Partial<Workflow> = {}): Workflow {
  return {
    id: 'wf-1',
    name: 'Test Workflow',
    agent_id: 'agent-1',
    source_config: {
      type: 'github_issues',
      owner: 'acme',
      repo: 'myrepo',
      labels: ['bug'],
      state: 'open',
    },
    prompt_template: 'Fix: {{title}}',
    poll_interval_secs: 900,
    enabled: true,
    tool_policy: { type: 'AllowAll' },
    created_at: '2024-01-01T00:00:00Z',
    updated_at: '2024-01-01T00:00:00Z',
    ...overrides,
  }
}

function makeDispatch(overrides: Partial<DispatchRecord> = {}): DispatchRecord {
  return {
    id: 'dr-1',
    workflow_id: 'wf-1',
    source_id: '42',
    agent_id: 'agent-1',
    prompt_sent: 'Fix: My bug',
    status: 'dispatched',
    dispatched_at: '2024-01-01T00:00:00Z',
    completed_at: undefined,
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// useWorkflows
// ---------------------------------------------------------------------------

describe('useWorkflows', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('fetches and returns workflows', async () => {
    const wf = makeWorkflow()
    vi.spyOn(orchestratorClient, 'listWorkflows').mockResolvedValue({
      items: [wf],
      total: 1,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useWorkflows())
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.workflows).toHaveLength(1)
    expect(result.current.workflows[0].name).toBe('Test Workflow')
    expect(result.current.total).toBe(1)
  })

  it('sets error on fetch failure', async () => {
    vi.spyOn(orchestratorClient, 'listWorkflows').mockRejectedValue(
      new Error('Service unavailable'),
    )

    const { result } = renderHook(() => useWorkflows())
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBe('Service unavailable')
    expect(result.current.workflows).toHaveLength(0)
  })

  it('filters by search term', async () => {
    const wf1 = makeWorkflow({ id: '1', name: 'GitHub Dispatch' })
    const wf2 = makeWorkflow({ id: '2', name: 'File Watch' })
    vi.spyOn(orchestratorClient, 'listWorkflows').mockResolvedValue({
      items: [wf1, wf2],
      total: 2,
      limit: 200,
      offset: 0,
    })

    const { result } = renderHook(() => useWorkflows({ search: 'GitHub' }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.workflows).toHaveLength(1)
    expect(result.current.workflows[0].name).toBe('GitHub Dispatch')
  })

  it('calls toggleEnabled with correct params', async () => {
    const wf = makeWorkflow({ enabled: true })
    vi.spyOn(orchestratorClient, 'listWorkflows').mockResolvedValue({
      items: [wf],
      total: 1,
      limit: 200,
      offset: 0,
    })
    const updatedWf = { ...wf, enabled: false }
    const updateSpy = vi
      .spyOn(orchestratorClient, 'updateWorkflow')
      .mockResolvedValue(updatedWf)

    const { result } = renderHook(() => useWorkflows())
    await waitFor(() => expect(result.current.loading).toBe(false))

    await result.current.toggleEnabled('wf-1', false)
    expect(updateSpy).toHaveBeenCalledWith('wf-1', { enabled: false })
  })

  it('deleteWorkflow removes workflow from state', async () => {
    const wf = makeWorkflow()
    vi.spyOn(orchestratorClient, 'listWorkflows').mockResolvedValue({
      items: [wf],
      total: 1,
      limit: 200,
      offset: 0,
    })
    vi.spyOn(orchestratorClient, 'deleteWorkflow').mockResolvedValue(undefined)

    const { result } = renderHook(() => useWorkflows())
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.workflows).toHaveLength(1)

    await result.current.deleteWorkflow('wf-1')
    await waitFor(() => expect(result.current.workflows).toHaveLength(0))
  })
})

// ---------------------------------------------------------------------------
// useDispatchHistory
// ---------------------------------------------------------------------------

describe('useDispatchHistory', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('fetches dispatch history', async () => {
    const dispatch = makeDispatch()
    vi.spyOn(orchestratorClient, 'getWorkflowHistory').mockResolvedValue({
      items: [dispatch],
      total: 1,
      limit: 100,
      offset: 0,
    })

    const { result } = renderHook(() => useDispatchHistory({ workflowId: 'wf-1' }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.dispatches).toHaveLength(1)
    expect(result.current.dispatches[0].source_id).toBe('42')
  })

  it('filters by status', async () => {
    const d1 = makeDispatch({ id: 'd1', status: 'dispatched' })
    const d2 = makeDispatch({ id: 'd2', status: 'completed' })
    vi.spyOn(orchestratorClient, 'getWorkflowHistory').mockResolvedValue({
      items: [d1, d2],
      total: 2,
      limit: 100,
      offset: 0,
    })

    const { result } = renderHook(() =>
      useDispatchHistory({ workflowId: 'wf-1', status: 'dispatched' }),
    )
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.dispatches).toHaveLength(1)
    expect(result.current.dispatches[0].status).toBe('dispatched')
  })

  it('sets error on failure', async () => {
    vi.spyOn(orchestratorClient, 'getWorkflowHistory').mockRejectedValue(
      new Error('Not found'),
    )

    const { result } = renderHook(() => useDispatchHistory({ workflowId: 'wf-1' }))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.error).toBe('Not found')
    expect(result.current.dispatches).toHaveLength(0)
  })
})
