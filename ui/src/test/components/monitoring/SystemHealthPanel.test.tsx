/**
 * Tests for SystemHealthPanel component.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SystemHealthPanel } from '@/components/monitoring/SystemHealthPanel'
import type { ServiceMetricsData } from '@/hooks/useMetrics'

// Mock the service clients used for health checks
vi.mock('@/services/orchestrator', () => ({
  orchestratorClient: { getHealth: vi.fn().mockResolvedValue({ status: 'ok' }) },
}))
vi.mock('@/services/notify', () => ({
  notifyClient: { getHealth: vi.fn().mockResolvedValue({ status: 'ok' }) },
}))
vi.mock('@/services/ask', () => ({
  askClient: { getHealth: vi.fn().mockResolvedValue({ status: 'ok' }) },
}))

function makeMetrics(overrides: Partial<ServiceMetricsData>[] = []): ServiceMetricsData[] {
  const defaults: ServiceMetricsData[] = [
    { key: 'orchestrator', name: 'Orchestrator', port: 17006, http: { requestsTotal: 100, errorsTotal: 0, errorRate: 0 }, reachable: true },
    { key: 'notify', name: 'Notify', port: 17004, http: { requestsTotal: 50, errorsTotal: 0, errorRate: 0 }, reachable: true },
    { key: 'ask', name: 'Ask', port: 17001, http: { requestsTotal: 20, errorsTotal: 0, errorRate: 0 }, reachable: false },
  ]
  return defaults.map((d, i) => ({ ...d, ...overrides[i] }))
}

describe('SystemHealthPanel', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders "System Health" heading', () => {
    render(<SystemHealthPanel serviceMetrics={makeMetrics()} />)
    expect(screen.getByText('System Health')).toBeTruthy()
  })

  it('renders a card for each service', () => {
    render(<SystemHealthPanel serviceMetrics={makeMetrics()} />)
    expect(screen.getByText('Orchestrator')).toBeTruthy()
    expect(screen.getByText('Notify')).toBeTruthy()
    expect(screen.getByText('Ask')).toBeTruthy()
  })

  it('shows port for each service', () => {
    render(<SystemHealthPanel serviceMetrics={makeMetrics()} />)
    expect(screen.getByText('Port 17006')).toBeTruthy()
    expect(screen.getByText('Port 17004')).toBeTruthy()
    expect(screen.getByText('Port 17001')).toBeTruthy()
  })

  it('shows loading skeletons when loading=true', () => {
    const { container } = render(<SystemHealthPanel serviceMetrics={[]} loading />)
    const pulses = container.querySelectorAll('.animate-pulse')
    expect(pulses.length).toBeGreaterThanOrEqual(3)
  })

  it('shows response time dash before first check', () => {
    render(<SystemHealthPanel serviceMetrics={makeMetrics()} />)
    // Before timers resolve, all show '—'
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBeGreaterThanOrEqual(1)
  })
})
