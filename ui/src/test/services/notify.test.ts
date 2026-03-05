import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { NotifyClient } from '@/services/notify'
import type { Notification } from '@/types/notify'

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

const mockNotification: Notification = {
  id: 'notif-uuid-1',
  source: 'AskService',
  lifetime: { type: 'Persistent' },
  priority: 'Normal',
  status: 'Pending',
  title: 'Test Notification',
  message: 'Something happened',
  requires_response: false,
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
}

const paginatedNotifications = {
  items: [mockNotification],
  total: 1,
  limit: 20,
  offset: 0,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('NotifyClient', () => {
  let client: NotifyClient

  beforeEach(() => {
    client = new NotifyClient({
      baseUrl: 'http://localhost:17004',
      maxRetries: 1,
    })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  describe('getHealth', () => {
    it('calls GET /health', async () => {
      mockFetch(200, { service: 'notify', version: '0.2.0', status: 'ok' })
      const result = await client.getHealth()
      expect(result.service).toBe('notify')
    })
  })

  describe('listNotifications', () => {
    it('returns paginated response', async () => {
      mockFetch(200, paginatedNotifications)
      const result = await client.listNotifications()
      expect(result.items).toHaveLength(1)
      expect(result.total).toBe(1)
    })

    it('passes status filter', async () => {
      mockFetch(200, { ...paginatedNotifications, items: [] })
      await client.listNotifications({ status: 'Pending' })

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('status=Pending')
    })
  })

  describe('createNotification', () => {
    it('calls POST /notifications', async () => {
      mockFetch(201, mockNotification)
      const result = await client.createNotification({
        source: 'System',
        lifetime: { type: 'Persistent' },
        priority: 'High',
        title: 'Alert',
        message: 'Critical update',
        requires_response: true,
      })
      expect(result.id).toBe('notif-uuid-1')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('POST')
      const body = JSON.parse(callInit.body as string)
      expect(body.priority).toBe('High')
    })
  })

  describe('getNotification', () => {
    it('calls GET /notifications/:id', async () => {
      mockFetch(200, mockNotification)
      const result = await client.getNotification('notif-uuid-1')
      expect(result.title).toBe('Test Notification')

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/notifications/notif-uuid-1')
    })
  })

  describe('updateNotification', () => {
    it('calls PUT /notifications/:id', async () => {
      const updated = { ...mockNotification, status: 'Viewed' as const }
      mockFetch(200, updated)
      const result = await client.updateNotification('notif-uuid-1', { status: 'Viewed' })
      expect(result.status).toBe('Viewed')

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('PUT')
    })
  })

  describe('deleteNotification', () => {
    it('calls DELETE /notifications/:id and returns void', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValue(
          new Response(null, {
            status: 204,
            headers: new Headers({ 'content-length': '0' }),
          }),
        ),
      )
      const result = await client.deleteNotification('notif-uuid-1')
      expect(result).toBeUndefined()

      const callInit = vi.mocked(fetch).mock.calls[0][1] as RequestInit
      expect(callInit.method).toBe('DELETE')
    })
  })

  describe('listActionable', () => {
    it('calls GET /notifications/actionable', async () => {
      mockFetch(200, paginatedNotifications)
      await client.listActionable()

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/notifications/actionable')
    })
  })

  describe('listHistory', () => {
    it('calls GET /notifications/history', async () => {
      mockFetch(200, paginatedNotifications)
      await client.listHistory()

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/notifications/history')
    })
  })

  describe('getCount', () => {
    it('calls GET /notifications/count and returns CountResponse', async () => {
      mockFetch(200, {
        total: 5,
        by_status: [
          { status: 'Pending', count: 3 },
          { status: 'Viewed', count: 2 },
        ],
      })
      const result = await client.getCount()
      expect(result.total).toBe(5)
      expect(result.by_status).toHaveLength(2)

      const calledUrl = vi.mocked(fetch).mock.calls[0][0] as string
      expect(calledUrl).toContain('/notifications/count')
    })
  })

  describe('ephemeral notification lifetime', () => {
    it('handles Ephemeral lifetime with expires_at', async () => {
      const ephemeral: Notification = {
        ...mockNotification,
        lifetime: { type: 'Ephemeral', expires_at: '2024-12-31T23:59:59Z' },
      }
      mockFetch(200, ephemeral)
      const result = await client.getNotification('notif-uuid-1')
      expect(result.lifetime).toEqual({ type: 'Ephemeral', expires_at: '2024-12-31T23:59:59Z' })
    })
  })
})
