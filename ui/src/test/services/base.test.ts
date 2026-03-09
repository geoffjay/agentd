import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { ApiClient } from '@/services/base'
import { ApiError } from '@/types/common'

// ---------------------------------------------------------------------------
// Concrete subclass for testing (exposes protected methods)
// ---------------------------------------------------------------------------

class TestClient extends ApiClient {
  testGet<T>(path: string, params?: Record<string, string | number | boolean | undefined>) {
    return this.get<T>(path, params)
  }
  testPost<T>(path: string, body?: unknown) {
    return this.post<T>(path, body)
  }
  testPut<T>(path: string, body?: unknown) {
    return this.put<T>(path, body)
  }
  testDelete<T>(path: string) {
    return this.delete<T>(path)
  }
}

// ---------------------------------------------------------------------------
// Fetch mock helpers
// ---------------------------------------------------------------------------

function mockFetch(status: number, body: unknown, headers: Record<string, string> = {}) {
  const responseHeaders = new Headers({
    'content-type': 'application/json',
    ...headers,
  })

  const response = new Response(JSON.stringify(body), {
    status,
    headers: responseHeaders,
  })

  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(response))
}

function mockFetchError(message = 'Network error') {
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError(message)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ApiClient', () => {
  let client: TestClient

  beforeEach(() => {
    client = new TestClient({
      baseUrl: 'http://localhost:9999',
      maxRetries: 1, // disable retries for most tests
      timeoutMs: 5000,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  // -------------------------------------------------------------------------
  // Successful responses
  // -------------------------------------------------------------------------

  describe('successful responses', () => {
    it('parses a JSON body on 200', async () => {
      mockFetch(200, { hello: 'world' })
      const result = await client.testGet<{ hello: string }>('/test')
      expect(result).toEqual({ hello: 'world' })
    })

    it('returns undefined on 204 No Content', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValue(
          new Response(null, {
            status: 204,
            headers: new Headers({ 'content-length': '0' }),
          }),
        ),
      )
      const result = await client.testDelete<void>('/resource/1')
      expect(result).toBeUndefined()
    })

    it('sends POST body as JSON', async () => {
      mockFetch(201, { id: '123' })
      await client.testPost('/resource', { name: 'test' })

      const fetchMock = vi.mocked(fetch)
      const callInit = fetchMock.mock.calls[0][1] as RequestInit
      expect(callInit.body).toBe('{"name":"test"}')
      expect((callInit.headers as Record<string, string>)['Content-Type']).toBe('application/json')
    })

    it('appends query params to the URL', async () => {
      mockFetch(200, { items: [], total: 0, limit: 10, offset: 0 })
      await client.testGet('/items', { limit: 10, offset: 0, status: 'Pending' })

      const fetchMock = vi.mocked(fetch)
      const calledUrl = fetchMock.mock.calls[0][0] as string
      expect(calledUrl).toContain('limit=10')
      expect(calledUrl).toContain('offset=0')
      expect(calledUrl).toContain('status=Pending')
    })

    it('omits undefined query params', async () => {
      mockFetch(200, {})
      await client.testGet('/items', { limit: 10, status: undefined })

      const fetchMock = vi.mocked(fetch)
      const calledUrl = fetchMock.mock.calls[0][0] as string
      expect(calledUrl).not.toContain('status')
    })
  })

  // -------------------------------------------------------------------------
  // Error handling
  // -------------------------------------------------------------------------

  describe('error handling', () => {
    it('throws ApiError on 4xx with error field', async () => {
      mockFetch(404, { error: 'Not found' })

      await expect(client.testGet('/missing')).rejects.toMatchObject({
        status: 404,
        message: 'Not found',
      })
    })

    it('throws ApiError on 500', async () => {
      mockFetch(500, { error: 'Internal server error' })

      await expect(client.testGet('/fail')).rejects.toBeInstanceOf(ApiError)
    })

    it('throws ApiError on network failure', async () => {
      mockFetchError('Failed to fetch')

      await expect(client.testGet('/fail')).rejects.toBeInstanceOf(ApiError)
    })
  })

  // -------------------------------------------------------------------------
  // Retry logic
  // -------------------------------------------------------------------------

  describe('retry logic', () => {
    it('retries on 503 and succeeds on second attempt', async () => {
      const okResponse = new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: new Headers({ 'content-type': 'application/json' }),
      })
      const errResponse = new Response(JSON.stringify({ error: 'unavailable' }), {
        status: 503,
        headers: new Headers({ 'content-type': 'application/json' }),
      })

      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValueOnce(errResponse).mockResolvedValueOnce(okResponse),
      )

      // Build a client with 2 retries and zero backoff for speed
      const retryClient = new TestClient({
        baseUrl: 'http://localhost:9999',
        maxRetries: 2,
        initialBackoffMs: 0,
      })

      const result = await retryClient.testGet<{ ok: boolean }>('/flaky')
      expect(result).toEqual({ ok: true })
      expect(vi.mocked(fetch)).toHaveBeenCalledTimes(2)
    })

    it('does not retry on 4xx errors', async () => {
      mockFetch(400, { error: 'Bad request' })

      const retryClient = new TestClient({
        baseUrl: 'http://localhost:9999',
        maxRetries: 3,
        initialBackoffMs: 0,
      })

      await expect(retryClient.testGet('/bad')).rejects.toBeInstanceOf(ApiError)
      expect(vi.mocked(fetch)).toHaveBeenCalledTimes(1)
    })
  })

  // -------------------------------------------------------------------------
  // onRequest interceptor
  // -------------------------------------------------------------------------

  describe('onRequest interceptor', () => {
    it('calls the onRequest hook and merges headers', async () => {
      mockFetch(200, { ok: true })

      const interceptedClient = new TestClient({
        baseUrl: 'http://localhost:9999',
        maxRetries: 1,
        onRequest: (init) => ({
          ...init,
          headers: {
            ...(init.headers as Record<string, string>),
            Authorization: 'Bearer token',
          },
        }),
      })

      await interceptedClient.testGet('/secure')

      const fetchMock = vi.mocked(fetch)
      const callInit = fetchMock.mock.calls[0][1] as RequestInit
      expect((callInit.headers as Record<string, string>)['Authorization']).toBe('Bearer token')
    })
  })

  // -------------------------------------------------------------------------
  // WebSocket helper
  // -------------------------------------------------------------------------

  describe('openWebSocket', () => {
    it('converts http to ws scheme', () => {
      class WsTestClient extends ApiClient {
        ws(path: string) {
          return this.openWebSocket(path)
        }
      }

      // Mock WebSocket constructor
      const MockWS = vi.fn()
      vi.stubGlobal('WebSocket', MockWS)

      const wsClient = new WsTestClient({ baseUrl: 'http://localhost:17006' })
      wsClient.ws('/ws/agent-1')

      expect(MockWS).toHaveBeenCalledWith('ws://localhost:17006/ws/agent-1')
    })

    it('converts https to wss scheme', () => {
      const MockWS = vi.fn()
      vi.stubGlobal('WebSocket', MockWS)

      class WsTestClient extends ApiClient {
        ws(path: string) {
          return this.openWebSocket(path)
        }
      }
      const wsClient = new WsTestClient({ baseUrl: 'https://agentd.example.com' })
      wsClient.ws('/stream')

      expect(MockWS).toHaveBeenCalledWith('wss://agentd.example.com/stream')
    })
  })
})
