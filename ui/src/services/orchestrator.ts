/**
 * Client for the Orchestrator service (default port 17006).
 *
 * Manages agent lifecycle, tool policies, approvals, and exposes
 * WebSocket helpers for real-time agent output streaming.
 */

import { ApiClient } from './base'
import { serviceConfig } from './config'
import type { HealthResponse, PaginatedResponse } from '@/types/common'
import type {
  Agent,
  ApprovalActionRequest,
  CreateAgentRequest,
  ListAgentsParams,
  ListApprovalsParams,
  PendingApproval,
  SendMessageRequest,
  SendMessageResponse,
  SetModelRequest,
  ToolPolicy,
  UpdatePolicyRequest,
} from '@/types/orchestrator'

export class OrchestratorClient extends ApiClient {
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>('/health')
  }

  // -------------------------------------------------------------------------
  // Agents
  // -------------------------------------------------------------------------

  listAgents(params?: ListAgentsParams): Promise<PaginatedResponse<Agent>> {
    return this.get<PaginatedResponse<Agent>>('/agents', params as Record<string, string>)
  }

  createAgent(request: CreateAgentRequest): Promise<Agent> {
    return this.post<Agent>('/agents', request)
  }

  getAgent(id: string): Promise<Agent> {
    return this.get<Agent>(`/agents/${id}`)
  }

  deleteAgent(id: string): Promise<Agent> {
    return this.delete<Agent>(`/agents/${id}`)
  }

  // -------------------------------------------------------------------------
  // Agent actions
  // -------------------------------------------------------------------------

  sendMessage(agentId: string, message: string): Promise<SendMessageResponse> {
    const body: SendMessageRequest = { content: message }
    return this.post<SendMessageResponse>(`/agents/${agentId}/message`, body)
  }

  updateModel(agentId: string, request: SetModelRequest): Promise<Agent> {
    return this.put<Agent>(`/agents/${agentId}/model`, request)
  }

  // -------------------------------------------------------------------------
  // Tool policy
  // -------------------------------------------------------------------------

  getPolicy(agentId: string): Promise<ToolPolicy> {
    return this.get<ToolPolicy>(`/agents/${agentId}/policy`)
  }

  updatePolicy(agentId: string, policy: UpdatePolicyRequest): Promise<ToolPolicy> {
    return this.put<ToolPolicy>(`/agents/${agentId}/policy`, policy)
  }

  // -------------------------------------------------------------------------
  // Approvals
  // -------------------------------------------------------------------------

  listApprovals(params?: ListApprovalsParams): Promise<PaginatedResponse<PendingApproval>> {
    return this.get<PaginatedResponse<PendingApproval>>(
      '/approvals',
      params as Record<string, string>,
    )
  }

  listAgentApprovals(
    agentId: string,
    params?: ListApprovalsParams,
  ): Promise<PaginatedResponse<PendingApproval>> {
    return this.get<PaginatedResponse<PendingApproval>>(
      `/agents/${agentId}/approvals`,
      params as Record<string, string>,
    )
  }

  getApproval(id: string): Promise<PendingApproval> {
    return this.get<PendingApproval>(`/approvals/${id}`)
  }

  approveRequest(id: string, body?: ApprovalActionRequest): Promise<PendingApproval> {
    return this.post<PendingApproval>(`/approvals/${id}/approve`, body ?? {})
  }

  denyRequest(id: string, body?: ApprovalActionRequest): Promise<PendingApproval> {
    return this.post<PendingApproval>(`/approvals/${id}/deny`, body ?? {})
  }

  // -------------------------------------------------------------------------
  // WebSocket streaming
  // -------------------------------------------------------------------------

  /**
   * Opens a WebSocket to stream output from a specific agent.
   * URL: ws://<host>/ws/<agentId>
   */
  connectAgentStream(agentId: string): WebSocket {
    return this.openWebSocket(`/ws/${agentId}`)
  }

  /**
   * Opens a WebSocket to monitor all agents.
   * URL: ws://<host>/stream
   */
  connectAllStream(): WebSocket {
    return this.openWebSocket('/stream')
  }

  /**
   * Opens a WebSocket to monitor a specific agent.
   * URL: ws://<host>/stream/<agentId>
   */
  connectAgentMonitor(agentId: string): WebSocket {
    return this.openWebSocket(`/stream/${agentId}`)
  }
}

/** Singleton client instance using the configured service URL */
export const orchestratorClient = new OrchestratorClient({
  baseUrl: serviceConfig.orchestratorServiceUrl,
})
