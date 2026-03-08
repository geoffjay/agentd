/**
 * useAgentDetail — hook for the agent detail page.
 *
 * Provides:
 * - Agent data fetching with configurable auto-refresh
 * - Loading / error state
 * - Agent actions: deleteAgent, sendMessage, updateModel, updatePolicy
 * - Pending approvals for this agent with approve/deny actions
 */

import { useCallback, useEffect, useState } from 'react'
import { orchestratorClient } from '@/services/orchestrator'
import type { Agent, PendingApproval, SetModelRequest, ToolPolicy } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface UseAgentDetailOptions {
  /** Auto-refresh interval in ms; 0 or undefined disables auto-refresh */
  refreshInterval?: number
  /** Pause auto-refresh (e.g. when a dialog is open) */
  paused?: boolean
}

export interface UseAgentDetailResult {
  agent: Agent | null
  loading: boolean
  error?: string
  refetch: () => void

  // Actions
  deleteAgent: () => Promise<void>
  sendMessage: (message: string) => Promise<void>
  updateModel: (request: SetModelRequest) => Promise<void>
  updatePolicy: (policy: ToolPolicy) => Promise<void>

  // Approvals
  approvals: PendingApproval[]
  approvalsLoading: boolean
  approvalsError?: string
  approveRequest: (approvalId: string) => Promise<void>
  denyRequest: (approvalId: string) => Promise<void>
  refetchApprovals: () => void
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

const DEFAULT_REFRESH_INTERVAL = 10_000

export function useAgentDetail(
  agentId: string,
  { refreshInterval = DEFAULT_REFRESH_INTERVAL, paused = false }: UseAgentDetailOptions = {},
): UseAgentDetailResult {
  const [agent, setAgent] = useState<Agent | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const [approvals, setApprovals] = useState<PendingApproval[]>([])
  const [approvalsLoading, setApprovalsLoading] = useState(true)
  const [approvalsError, setApprovalsError] = useState<string | undefined>()

  // ---------------------------------------------------------------------------
  // Agent fetch
  // ---------------------------------------------------------------------------

  const fetchAgent = useCallback(
    async (showLoading = true) => {
      if (showLoading) {
        setLoading(true)
        setError(undefined)
      }
      try {
        const data = await orchestratorClient.getAgent(agentId)
        setAgent(data)
        setError(undefined)
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to load agent'
        setError(msg)
      } finally {
        setLoading(false)
      }
    },
    [agentId],
  )

  // ---------------------------------------------------------------------------
  // Approvals fetch
  // ---------------------------------------------------------------------------

  const fetchApprovals = useCallback(
    async (showLoading = true) => {
      if (showLoading) {
        setApprovalsLoading(true)
        setApprovalsError(undefined)
      }
      try {
        const result = await orchestratorClient.listAgentApprovals(agentId, {
          status: 'pending',
          limit: 50,
        })
        setApprovals(result.items)
        setApprovalsError(undefined)
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to load approvals'
        setApprovalsError(msg)
      } finally {
        setApprovalsLoading(false)
      }
    },
    [agentId],
  )

  // ---------------------------------------------------------------------------
  // Initial fetches
  // ---------------------------------------------------------------------------

  useEffect(() => {
    fetchAgent(true)
    fetchApprovals(true)
  }, [fetchAgent, fetchApprovals])

  // ---------------------------------------------------------------------------
  // Auto-refresh
  // ---------------------------------------------------------------------------

  useEffect(() => {
    if (!refreshInterval || paused) return

    const timer = setInterval(() => {
      fetchAgent(false)
      fetchApprovals(false)
    }, refreshInterval)

    return () => clearInterval(timer)
  }, [refreshInterval, paused, fetchAgent, fetchApprovals])

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  const deleteAgent = useCallback(async () => {
    await orchestratorClient.deleteAgent(agentId)
  }, [agentId])

  const sendMessage = useCallback(
    async (message: string) => {
      await orchestratorClient.sendMessage(agentId, message)
    },
    [agentId],
  )

  const updateModel = useCallback(
    async (request: SetModelRequest) => {
      const updated = await orchestratorClient.updateModel(agentId, request)
      setAgent(updated)
    },
    [agentId],
  )

  const updatePolicy = useCallback(
    async (policy: ToolPolicy) => {
      const updated = await orchestratorClient.updatePolicy(agentId, policy)
      // Reflect updated policy in local agent state
      setAgent((prev) =>
        prev ? { ...prev, config: { ...prev.config, tool_policy: updated } } : prev,
      )
    },
    [agentId],
  )

  const approveRequest = useCallback(async (approvalId: string) => {
    await orchestratorClient.approveRequest(approvalId)
    setApprovals((prev) => prev.filter((a) => a.id !== approvalId))
  }, [])

  const denyRequest = useCallback(async (approvalId: string) => {
    await orchestratorClient.denyRequest(approvalId)
    setApprovals((prev) => prev.filter((a) => a.id !== approvalId))
  }, [])

  return {
    agent,
    loading,
    error,
    refetch: () => fetchAgent(false),

    deleteAgent,
    sendMessage,
    updateModel,
    updatePolicy,

    approvals,
    approvalsLoading,
    approvalsError,
    approveRequest,
    denyRequest,
    refetchApprovals: () => fetchApprovals(false),
  }
}
