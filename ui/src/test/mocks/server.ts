/**
 * MSW node server for Vitest.
 *
 * Used in setup.ts to intercept all fetch requests during tests.
 * The server is started before all tests, reset between each test
 * (so per-test handler overrides don't leak), and closed after all tests.
 *
 * To override a handler for a single test:
 *
 *   import { server } from '@/test/mocks/server'
 *   import { http, HttpResponse } from 'msw'
 *
 *   server.use(
 *     http.get('http://localhost:17006/agents', () =>
 *       HttpResponse.json({ items: [], total: 0, limit: 20, offset: 0 })
 *     )
 *   )
 */

import { setupServer } from 'msw/node'
import { handlers } from './handlers'

export const server = setupServer(...handlers)
