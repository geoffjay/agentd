/**
 * Integration test for useServiceHealth hook.
 *
 * Uses MSW to intercept real fetch calls and verifies that the hook
 * correctly parses health responses from all four services.
 *
 * Note: the ApiClient retries on 5xx errors. We use 4xx errors in tests
 * to avoid slow retries; the catch block in fetchHealth() treats any error
 * as "down".
 */

import { describe, it, expect } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { server } from '@/test/mocks/server'
import { useServiceHealth } from '@/hooks/useServiceHealth'

describe('useServiceHealth (MSW integration)', () => {
  it('fetches health from all four services and marks them healthy', async () => {
    const { result } = renderHook(() => useServiceHealth())

    await waitFor(() => expect(result.current.initializing).toBe(false))

    expect(result.current.services).toHaveLength(4)
    const statuses = result.current.services.map((s) => s.status)
    expect(statuses.every((s) => s === 'healthy')).toBe(true)
  })

  it('includes service names Orchestrator, Notify, Ask, Memory', async () => {
    const { result } = renderHook(() => useServiceHealth())
    await waitFor(() => expect(result.current.initializing).toBe(false))

    const names = result.current.services.map((s) => s.name)
    expect(names).toContain('Orchestrator')
    expect(names).toContain('Notify')
    expect(names).toContain('Ask')
    expect(names).toContain('Memory')
  })

  it('marks orchestrator as down on 4xx response (no retry)', async () => {
    server.use(
      http.get('http://localhost:17006/health', () =>
        HttpResponse.json({ error: 'Not found' }, { status: 404 }),
      ),
    )

    const { result } = renderHook(() => useServiceHealth())
    await waitFor(() => expect(result.current.initializing).toBe(false))

    const orchestrator = result.current.services.find((s) => s.key === 'orchestrator')
    expect(orchestrator?.status).toBe('down')
    const notify = result.current.services.find((s) => s.key === 'notify')
    expect(notify?.status).toBe('healthy')
  })

  it('includes version string from health response', async () => {
    server.use(
      http.get('http://localhost:17006/health', () =>
        HttpResponse.json({ status: 'ok', version: '1.2.3' }),
      ),
    )

    const { result } = renderHook(() => useServiceHealth())
    await waitFor(() => expect(result.current.initializing).toBe(false))

    const orchestrator = result.current.services.find((s) => s.key === 'orchestrator')
    expect(orchestrator?.version).toBe('1.2.3')
  })

  it('sets lastChecked to a recent date', async () => {
    const before = Date.now()
    const { result } = renderHook(() => useServiceHealth())

    await waitFor(() => expect(result.current.initializing).toBe(false))

    const orchestrator = result.current.services.find((s) => s.key === 'orchestrator')
    expect(orchestrator?.lastChecked).toBeDefined()
    expect(orchestrator!.lastChecked!.getTime()).toBeGreaterThanOrEqual(before)
  })
})
