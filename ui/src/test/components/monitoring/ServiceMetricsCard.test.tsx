/**
 * Tests for ServiceMetricsCard component.
 */

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ServiceMetricsCard } from '@/components/monitoring/ServiceMetricsCard'
import type { ServiceMetricsData } from '@/hooks/useMetrics'

function makeServiceData(overrides: Partial<ServiceMetricsData> = {}): ServiceMetricsData {
  return {
    key: 'orchestrator',
    name: 'Orchestrator',
    port: 17006,
    http: { requestsTotal: 500, errorsTotal: 5, errorRate: 0.01 },
    reachable: true,
    ...overrides,
  }
}

describe('ServiceMetricsCard', () => {
  it('renders service name and port', () => {
    render(<ServiceMetricsCard data={makeServiceData()} />)
    expect(screen.getByText('Orchestrator')).toBeTruthy()
    expect(screen.getByText('Port 17006')).toBeTruthy()
  })

  it('shows "Up" badge when reachable', () => {
    render(<ServiceMetricsCard data={makeServiceData({ reachable: true })} />)
    expect(screen.getByText('Up')).toBeTruthy()
  })

  it('shows "Down" badge when not reachable', () => {
    render(<ServiceMetricsCard data={makeServiceData({ reachable: false })} />)
    expect(screen.getByText('Down')).toBeTruthy()
  })

  it('shows "Service unreachable" message when down', () => {
    render(<ServiceMetricsCard data={makeServiceData({ reachable: false })} />)
    expect(screen.getByText('Service unreachable')).toBeTruthy()
  })

  it('shows request count when reachable', () => {
    render(<ServiceMetricsCard data={makeServiceData({ http: { requestsTotal: 1500, errorsTotal: 0, errorRate: 0 } })} />)
    expect(screen.getByText('1.5k')).toBeTruthy()
  })

  it('shows response time when provided', () => {
    render(<ServiceMetricsCard data={makeServiceData()} responseTimeMs={42} />)
    expect(screen.getByText('42ms')).toBeTruthy()
    expect(screen.getByText('Fast')).toBeTruthy()
  })

  it('shows dash when no response time', () => {
    render(<ServiceMetricsCard data={makeServiceData()} />)
    expect(screen.getByText('—')).toBeTruthy()
  })

  it('shows error count when there are errors', () => {
    render(<ServiceMetricsCard data={makeServiceData({ http: { requestsTotal: 100, errorsTotal: 3, errorRate: 0.03 } })} />)
    expect(screen.getByText(/3 errors? logged/)).toBeTruthy()
  })

  it('renders loading skeleton when loading=true', () => {
    const { container } = render(<ServiceMetricsCard data={makeServiceData()} loading />)
    expect(container.querySelector('[aria-busy="true"]')).toBeTruthy()
  })

  it('labels slow response correctly', () => {
    render(<ServiceMetricsCard data={makeServiceData()} responseTimeMs={600} />)
    expect(screen.getByText('Slow')).toBeTruthy()
  })

  it('labels OK response correctly', () => {
    render(<ServiceMetricsCard data={makeServiceData()} responseTimeMs={200} />)
    expect(screen.getByText('OK')).toBeTruthy()
  })
})
