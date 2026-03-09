import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { AskClient } from '@/services/ask'

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
// Tests
// ---------------------------------------------------------------------------

describe('AskClient', () => {
  let client: AskClient

  beforeEach(() => {
    client = new AskClient({
      baseUrl: 'http://localhost:17001',
      maxRetries: 1,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  describe('getHealth', () => {
    it('calls GET /health', async () => {
      mockFetch(200, { service: 'ask', version: '0.2.0', status: 'ok' })
      const result = await client.getHealth()
      expect(result.service).toBe('ask')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/health')
    })
  })

  describe('trigger', () => {
    it('calls POST /trigger and returns TriggerResponse', async () => {
      const mockResponse = {
        checks_run: ['tmux_sessions'],
        notifications_sent: ['notif-uuid-1'],
        results: {
          tmux_sessions: {
            running: true,
            session_count: 2,
            sessions: ['main', 'dev'],
          },
        },
      }
      mockFetch(200, mockResponse)
      const result = await client.trigger()
      expect(result.checks_run).toContain('tmux_sessions')
      expect(result.results.tmux_sessions.running).toBe(true)
      expect(result.results.tmux_sessions.session_count).toBe(2)

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('POST')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/trigger')
    })

    it('handles no tmux sessions running', async () => {
      mockFetch(200, {
        checks_run: ['tmux_sessions'],
        notifications_sent: [],
        results: {
          tmux_sessions: {
            running: false,
            session_count: 0,
            sessions: null,
          },
        },
      })
      const result = await client.trigger()
      expect(result.results.tmux_sessions.running).toBe(false)
      expect(result.results.tmux_sessions.session_count).toBe(0)
      expect(result.notifications_sent).toHaveLength(0)
    })
  })

  describe('answer', () => {
    it('calls POST /answer with the request body', async () => {
      mockFetch(200, {
        success: true,
        message: 'Answer recorded',
        question_id: 'q-uuid-1',
      })
      const result = await client.answer({
        question_id: 'q-uuid-1',
        answer: 'yes',
      })
      expect(result.success).toBe(true)
      expect(result.question_id).toBe('q-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('POST')
      const body = JSON.parse(callInit.body as string)
      expect(body.question_id).toBe('q-uuid-1')
      expect(body.answer).toBe('yes')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/answer')
    })

    it('handles failure response', async () => {
      mockFetch(200, {
        success: false,
        message: 'Question already answered',
        question_id: 'q-uuid-1',
      })
      const result = await client.answer({ question_id: 'q-uuid-1', answer: 'no' })
      expect(result.success).toBe(false)
    })
  })
})
