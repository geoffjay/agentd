import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { OrchestratorClient } from '@/services/orchestrator'
import { ApiError } from '@/types/common'
import type { Agent, PendingApproval } from '@/types/orchestrator'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeJsonResponse(status: number, body: unknown) {
  return new Response(JSON.stringify(body), {
    status,
    headers: new Headers({ 'content-type': 'application/json' }),
  })
}

function mockFetch(status: number, body: unknown) {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(makeJsonResponse(status, body)))
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const mockAgent: Agent = {
  id: 'agent-uuid-1',
  name: 'test-agent',
  status: 'Running',
  config: {
    working_dir: '/tmp',
    shell: 'bash',
    interactive: false,
    tool_policy: { type: 'AllowAll' },
  },
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
}

const mockApproval: PendingApproval = {
  id: 'approval-uuid-1',
  agent_id: 'agent-uuid-1',
  request_id: 'req-1',
  tool_name: 'bash',
  tool_input: { command: 'ls' },
  status: 'Pending',
  created_at: '2024-01-01T00:00:00Z',
  expires_at: '2024-01-01T00:10:00Z',
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('OrchestratorClient', () => {
  let client: OrchestratorClient

  beforeEach(() => {
    client = new OrchestratorClient({
      baseUrl: 'http://localhost:17006',
      maxRetries: 1,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  describe('getHealth', () => {
    it('calls GET /health', async () => {
      mockFetch(200, { service: 'orchestrator', version: '0.2.0', status: 'ok' })
      const result = await client.getHealth()
      expect(result.service).toBe('orchestrator')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/health')
    })
  })

  describe('listAgents', () => {
    it('calls GET /agents and returns paginated response', async () => {
      mockFetch(200, { items: [mockAgent], total: 1, limit: 20, offset: 0 })
      const result = await client.listAgents()
      expect(result.items).toHaveLength(1)
      expect(result.items[0].id).toBe('agent-uuid-1')
    })

    it('passes status filter as query param', async () => {
      mockFetch(200, { items: [], total: 0, limit: 20, offset: 0 })
      await client.listAgents({ status: 'Running' })

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('status=Running')
    })
  })

  describe('createAgent', () => {
    it('calls POST /agents with the request body', async () => {
      mockFetch(201, mockAgent)
      const request = {
        name: 'test-agent',
        working_dir: '/tmp',
        shell: 'bash',
        interactive: false,
        tool_policy: { type: 'AllowAll' as const },
      }
      const result = await client.createAgent(request)
      expect(result.id).toBe('agent-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      const body = JSON.parse(callInit.body as string)
      expect(body.name).toBe('test-agent')
    })
  })

  describe('getAgent', () => {
    it('calls GET /agents/:id', async () => {
      mockFetch(200, mockAgent)
      const result = await client.getAgent('agent-uuid-1')
      expect(result.status).toBe('Running')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/agents/agent-uuid-1')
    })

    it('throws ApiError on 404', async () => {
      mockFetch(404, { error: 'Agent not found' })
      await expect(client.getAgent('missing')).rejects.toMatchObject({
        status: 404,
        message: 'Agent not found',
      })
    })
  })

  describe('deleteAgent', () => {
    it('calls DELETE /agents/:id', async () => {
      mockFetch(200, mockAgent)
      await client.deleteAgent('agent-uuid-1')

      const fetchMock = vi.mocked(fetch)
      const callInit = fetchMock.mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('DELETE')
    })
  })

  describe('sendMessage', () => {
    it('calls POST /agents/:id/message', async () => {
      mockFetch(200, { status: 'ok', agent_id: 'agent-uuid-1' })
      const result = await client.sendMessage('agent-uuid-1', 'Hello agent')
      expect(result.status).toBe('ok')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      const body = JSON.parse(callInit.body as string)
      expect(body.content).toBe('Hello agent')
    })
  })

  describe('getPolicy / updatePolicy', () => {
    it('calls GET /agents/:id/policy', async () => {
      mockFetch(200, { type: 'AllowAll' })
      const policy = await client.getPolicy('agent-uuid-1')
      expect(policy.type).toBe('AllowAll')
    })

    it('calls PUT /agents/:id/policy', async () => {
      const newPolicy = { type: 'RequireApproval' as const }
      mockFetch(200, newPolicy)
      const result = await client.updatePolicy('agent-uuid-1', newPolicy)
      expect(result.type).toBe('RequireApproval')
    })
  })

  describe('approvals', () => {
    it('listApprovals calls GET /approvals', async () => {
      mockFetch(200, { items: [mockApproval], total: 1, limit: 20, offset: 0 })
      const result = await client.listApprovals()
      expect(result.items[0].tool_name).toBe('bash')
    })

    it('approveRequest calls POST /approvals/:id/approve', async () => {
      mockFetch(200, { ...mockApproval, status: 'Approved' })
      const result = await client.approveRequest('approval-uuid-1')
      expect(result.status).toBe('Approved')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/approvals/approval-uuid-1/approve')
    })

    it('denyRequest calls POST /approvals/:id/deny', async () => {
      mockFetch(200, { ...mockApproval, status: 'Denied' })
      const result = await client.denyRequest('approval-uuid-1', { reason: 'Too dangerous' })
      expect(result.status).toBe('Denied')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      const body = JSON.parse(callInit.body as string)
      expect(body.reason).toBe('Too dangerous')
    })
  })

  describe('WebSocket helpers', () => {
    it('connectAgentStream opens ws://.../ws/:id', () => {
      const MockWS = vi.fn()
      vi.stubGlobal('WebSocket', MockWS)
      client.connectAgentStream('agent-uuid-1')
      expect(MockWS).toHaveBeenCalledWith('ws://localhost:17006/ws/agent-uuid-1')
    })

    it('connectAllStream opens ws://.../stream', () => {
      const MockWS = vi.fn()
      vi.stubGlobal('WebSocket', MockWS)
      client.connectAllStream()
      expect(MockWS).toHaveBeenCalledWith('ws://localhost:17006/stream')
    })
  })

  describe('error propagation', () => {
    it('throws ApiError with correct status and message', async () => {
      mockFetch(422, { error: 'Invalid config' })
      let caught: ApiError | null = null
      try {
        await client.createAgent({
          name: '',
          working_dir: '',
          shell: '',
          interactive: false,
          tool_policy: { type: 'AllowAll' },
        })
      } catch (e) {
        caught = e as ApiError
      }
      expect(caught).toBeInstanceOf(ApiError)
      expect(caught?.status).toBe(422)
      expect(caught?.message).toBe('Invalid config')
    })
  })
})
