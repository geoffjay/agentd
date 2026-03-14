/**
 * MSW request handlers for the Memory service (port 17008).
 *
 * Provides default responses for all Memory API endpoints.
 * Override per test with server.use().
 */

import { http, HttpResponse } from 'msw'
import { makeMemory, makeMemoryList, makeSearchResponse } from '../factories'
import type { PaginatedResponse } from '@/types/common'
import type { Memory } from '@/types/memory'

const BASE = 'http://localhost:17008'

// Default dataset shared by all handlers (reset per test via factories)
const DEFAULT_MEMORIES = makeMemoryList(3)

function paginated<T>(items: T[], total?: number): PaginatedResponse<T> {
  return { items, total: total ?? items.length, limit: 50, offset: 0 }
}

export const memoryHandlers = [
  // -------------------------------------------------------------------------
  // Health
  // -------------------------------------------------------------------------

  http.get(`${BASE}/health`, () =>
    HttpResponse.json({ status: 'ok', service: 'memory', version: '0.2.0' }),
  ),

  // -------------------------------------------------------------------------
  // Memories CRUD
  // -------------------------------------------------------------------------

  http.get(`${BASE}/memories`, () =>
    HttpResponse.json(paginated<Memory>(DEFAULT_MEMORIES)),
  ),

  http.post(`${BASE}/memories`, async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const memory = makeMemory({
      content: String(body.content ?? 'New memory'),
      created_by: String(body.created_by ?? 'user-1'),
      type: (body.type as Memory['type']) ?? 'information',
      visibility: (body.visibility as Memory['visibility']) ?? 'public',
    })
    return HttpResponse.json(memory, { status: 201 })
  }),

  http.get(`${BASE}/memories/:id`, ({ params }) => {
    const memory =
      DEFAULT_MEMORIES.find((m) => m.id === params.id) ??
      makeMemory({ id: String(params.id) })
    return HttpResponse.json(memory)
  }),

  http.delete(`${BASE}/memories/:id`, () =>
    HttpResponse.json({ deleted: true }),
  ),

  // -------------------------------------------------------------------------
  // Visibility
  // -------------------------------------------------------------------------

  http.put(`${BASE}/memories/:id/visibility`, async ({ params, request }) => {
    const body = (await request.json()) as Record<string, unknown>
    const existing =
      DEFAULT_MEMORIES.find((m) => m.id === params.id) ??
      makeMemory({ id: String(params.id) })
    return HttpResponse.json({
      ...existing,
      visibility: body.visibility ?? existing.visibility,
      shared_with: (body.shared_with as string[]) ?? existing.shared_with,
    })
  }),

  // -------------------------------------------------------------------------
  // Search
  // -------------------------------------------------------------------------

  http.post(`${BASE}/memories/search`, () =>
    HttpResponse.json(makeSearchResponse(DEFAULT_MEMORIES.slice(0, 2))),
  ),
]
